use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rikka_proton_api::{Api, Environment, Session};
use serde_json::Value;

use rikka_test_proton::{Request, Response, binary_response, not_found, serve};

const UID: &str = "a1b2c3d4e5";
const UNREACHABLE_HOST: &str = "127.0.0.2";
const UNREACHABLE_HOST_BASE32: &str = "GEZDOLRQFYYC4MQ";
const ALTERNATIVE_HOST: &str = "127.0.0.1";
const SESSION_COOKIE: &str = "Session-Id=routed-session";

#[derive(Debug, Default)]
struct DnsLog {
    questions: Vec<String>,
    accepts: Vec<Option<String>>,
    cookies: Vec<Option<String>>,
}

fn question(query: &[u8]) -> String {
    let mut labels = Vec::new();
    let mut offset = 12;
    while let Some(&length) = query.get(offset)
        && length > 0
    {
        let label = &query[offset + 1..offset + 1 + usize::from(length)];
        labels.push(String::from_utf8_lossy(label).into_owned());
        offset += 1 + usize::from(length);
    }
    labels.join(".")
}

fn text_reply(query: &[u8], text: &str) -> Vec<u8> {
    let mut reply = Vec::new();
    reply.extend_from_slice(&query[..2]);
    reply.extend_from_slice(&[0x81, 0x80, 0, 1, 0, 1, 0, 0, 0, 0]);
    reply.extend_from_slice(&query[12..]);

    let length = u8::try_from(text.len()).unwrap();
    reply.extend_from_slice(&[0xc0, 0x0c, 0, 16, 0, 1, 0, 0, 0, 60, 0, length + 1, length]);
    reply.extend_from_slice(text.as_bytes());
    reply
}

#[tokio::test(flavor = "multi_thread")]
async fn unreachable_api_uses_doh_route() {
    let log = Arc::new(Mutex::new(DnsLog::default()));
    let server_log = log.clone();

    let address = serve(move |request: Request| {
        let mut log = server_log.lock().unwrap();
        match request.path.as_str() {
            "/dns-query" => {
                let query = URL_SAFE_NO_PAD
                    .decode(request.query("dns").unwrap())
                    .unwrap();
                log.questions.push(question(&query));
                log.accepts
                    .push(request.header("accept").map(str::to_owned));
                binary_response(200, text_reply(&query, ALTERNATIVE_HOST))
            }
            "/api/core/v4/users" => {
                log.cookies
                    .push(request.header("cookie").map(str::to_owned));
                Response::new(200, r#"{"Code":1000,"User":"routed"}"#)
                    .header(
                        "Set-Cookie",
                        &format!("{SESSION_COOKIE}; Domain=proton.me; Path=/; Secure; HttpOnly"),
                    )
                    .into_future()
            }
            _ => not_found(),
        }
    })
    .await;
    let port = address.rsplit(':').next().unwrap();

    let api = Api::builder()
        .app_version("linux-pass@1.0.0")
        .environment(Environment::custom(format!("http://{UNREACHABLE_HOST}:{port}/api/")).unwrap())
        .session(Session::new(UID, "access", "refresh"))
        .alternative_routing(true)
        .doh_providers([format!("{address}/dns-query/")])
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    for _ in 0..2 {
        let response: Value = api.get("core/v4/users").send().await.unwrap();
        assert_eq!(response["User"], "routed");
    }

    let log = log.lock().unwrap();
    assert_eq!(
        log.questions,
        [format!("{UID}.d{UNREACHABLE_HOST_BASE32}.protonpro.xyz")]
    );
    assert_eq!(log.accepts, [Some("application/dns-message".to_owned())]);
    assert_eq!(
        log.cookies,
        [None, Some(SESSION_COOKIE.to_owned())],
        "Session-Id cookie not sent on routed requests"
    );
}
