use std::sync::{Arc, Mutex};

use rikka_omnibox::{Error, SearchEngine, Suggester};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug, Clone, Copy)]
enum Reply {
    Json(&'static str),
    Huge,
    Status(u16),
}

async fn serve(reply: Reply) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&requests);
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let seen = Arc::clone(&seen);
            tokio::spawn(async move {
                let mut head = Vec::new();
                let mut buffer = [0u8; 4096];
                while !head.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut buffer).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => head.extend_from_slice(&buffer[..read]),
                    }
                }
                seen.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&head).into_owned());
                let response = match reply {
                    Reply::Json(body) => format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\nset-cookie: tracker=1; Path=/\r\ncontent-length: {}\r\n\r\n{body}",
                        body.len()
                    ),
                    Reply::Huge => {
                        let body = format!("[\"q\",[\"{}\"]]", "x".repeat(200_000));
                        format!(
                            "HTTP/1.1 200 OK\r\ntransfer-encoding: identity\r\nconnection: close\r\n\r\n{body}"
                        )
                    }
                    Reply::Status(code) => {
                        format!("HTTP/1.1 {code} Nope\r\ncontent-length: 0\r\n\r\n")
                    }
                };
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    (format!("http://{address}"), requests)
}

fn engine(base: &str) -> SearchEngine {
    SearchEngine::builder("local")
        .name("Local")
        .keyword("local")
        .search_url(format!("{base}/search?q=%s"))
        .suggestion_url(format!("{base}/complete?q={{searchTerms}}&type=list"))
        .build()
        .unwrap()
}

#[tokio::test]
async fn fetches_and_parses_suggestions_without_cookies() {
    let (base, requests) = serve(Reply::Json(
        r#"["café au lait",["café au lait recipe","café au lait 作り方"]]"#,
    ))
    .await;
    let suggester = Suggester::builder()
        .accept_language("ja,en;q=0.8")
        .build()
        .unwrap();
    let engine = engine(&base);
    let first = suggester.suggest(&engine, "café au lait").await.unwrap();
    let texts: Vec<&str> = first
        .iter()
        .map(|suggestion| suggestion.text.as_str())
        .collect();
    assert_eq!(texts, ["café au lait recipe", "café au lait 作り方"]);
    suggester.suggest(&engine, "café").await.unwrap();
    let requests = requests.lock().unwrap();
    assert!(requests[0].starts_with("GET /complete?q=caf%C3%A9+au+lait&type=list HTTP/1.1"));
    assert!(
        requests[0]
            .to_ascii_lowercase()
            .contains("accept-language: ja,en;q=0.8")
    );
    assert!(
        requests
            .iter()
            .all(|request| !request.to_ascii_lowercase().contains("cookie:"))
    );
    assert!(
        requests
            .iter()
            .all(|request| !request.to_ascii_lowercase().contains("referer:"))
    );
}

#[tokio::test]
async fn caps_oversized_responses() {
    let (base, _) = serve(Reply::Huge).await;
    let result = Suggester::new().unwrap().suggest(&engine(&base), "q").await;
    assert!(matches!(result, Err(Error::TooLarge(65_536))), "{result:?}");
}

#[tokio::test]
async fn reports_error_statuses() {
    let (base, _) = serve(Reply::Status(503)).await;
    let result = Suggester::new().unwrap().suggest(&engine(&base), "q").await;
    assert!(matches!(result, Err(Error::Status(503))), "{result:?}");
}
