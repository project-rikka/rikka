use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rikka_net::Url;
use rikka_oauth::{Client, Loopback, Pkce, Redirect};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn visit(redirect: &Redirect, target: &str) -> String {
    let port = Url::parse(redirect.uri())
        .expect("a redirect URL")
        .port()
        .expect("a port");
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("the redirect listens");
    stream
        .write_all(format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
        .await
        .expect("the request was sent");

    let mut answer = String::new();
    stream
        .read_to_string(&mut answer)
        .await
        .expect("the redirect answered");
    answer
}

#[test]
fn challenge_is_rfc7636_s256() {
    let pkce = Pkce::new();
    assert_eq!(pkce.verifier().len(), 128);
    assert_eq!(
        pkce.challenge(),
        URL_SAFE_NO_PAD.encode(Sha256::digest(pkce.verifier().as_bytes()))
    );
    assert_eq!(pkce.method(), "S256");
}

#[test]
fn authorization_url_binds_state_and_challenge() {
    let client = Client::new("rikka", "https://auth.example.com/token").expect("a client");
    let authorization = client
        .authorization(
            "https://auth.example.com/authorize",
            "http://localhost:1455/cb",
        )
        .scope("openid email")
        .pkce()
        .param("prompt", "login")
        .build()
        .expect("an authorization");

    let url = Url::parse(authorization.url()).expect("a URL");
    let query: HashMap<_, _> = url.query_pairs().into_owned().collect();

    assert_eq!(url.path(), "/authorize");
    assert_eq!(query["client_id"], "rikka");
    assert_eq!(query["response_type"], "code");
    assert_eq!(query["redirect_uri"], "http://localhost:1455/cb");
    assert_eq!(query["scope"], "openid email");
    assert_eq!(query["state"], authorization.state());
    assert_eq!(query["code_challenge_method"], "S256");
    assert_eq!(query["prompt"], "login");
}

#[tokio::test]
async fn loopback_accepts_only_matching_state() {
    let redirect = Loopback::ip(0)
        .path("/callback")
        .bind()
        .await
        .expect("a free port");
    assert!(redirect.uri().starts_with("http://127.0.0.1:"));
    assert!(redirect.uri().ends_with("/callback"));

    let waiting = redirect.code("s1");
    let visiting = async {
        let missing = visit(&redirect, "/favicon.ico").await;
        let stray = visit(&redirect, "/callback?code=x&state=other").await;
        let ours = visit(&redirect, "/callback/?code=abc&state=s1").await;
        (missing, stray, ours)
    };

    let (code, (missing, stray, ours)) = tokio::join!(waiting, visiting);

    assert_eq!(code.expect("the sign-in finished"), "abc");
    assert!(missing.starts_with("HTTP/1.1 404"));
    assert!(stray.starts_with("HTTP/1.1 400"));
    assert!(ours.starts_with("HTTP/1.1 200"));
}
