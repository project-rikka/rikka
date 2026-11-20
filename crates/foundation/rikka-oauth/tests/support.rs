use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rikka_net::Url;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

pub(crate) struct Asked {
    pub(crate) target: String,
    pub(crate) headers: HashMap<String, String>,
    pub(crate) body: String,
}

pub(crate) struct Site {
    pub(crate) base: String,
    targets: Arc<Mutex<Vec<String>>>,
}

pub(crate) async fn serve(
    status: &'static str,
    answer: Value,
) -> (String, tokio::task::JoinHandle<Asked>) {
    let (listener, base) = listen().await;
    let request = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("a request");
        answered(stream, |_| (status, answer)).await
    });
    (format!("{base}/token"), request)
}

impl Site {
    pub(crate) async fn start(
        pages: impl Fn(&str, &str) -> Option<Value> + Send + Sync + 'static,
    ) -> Site {
        let (listener, base) = listen().await;
        let targets = Arc::new(Mutex::new(Vec::new()));

        let asked = Arc::clone(&targets);
        let origin = base.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let request = answered(stream, |target| match pages(&origin, target) {
                    Some(page) => ("200 OK", page),
                    None => ("404 Not Found", json!({})),
                })
                .await;
                asked.lock().expect("recorded targets").push(request.target);
            }
        });
        Site { base, targets }
    }

    pub(crate) fn targets(&self) -> Vec<String> {
        self.targets.lock().expect("recorded targets").clone()
    }
}

pub(crate) fn form(body: &str) -> HashMap<String, String> {
    Url::parse(&format!("http://localhost/?{body}"))
        .expect("a form")
        .query_pairs()
        .into_owned()
        .collect()
}

async fn listen() -> (TcpListener, String) {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("a free port");
    let base = format!(
        "http://127.0.0.1:{}",
        listener.local_addr().expect("an address").port()
    );
    (listener, base)
}

async fn answered(
    stream: TcpStream,
    answering: impl FnOnce(&str) -> (&'static str, Value),
) -> Asked {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .await
        .expect("a request line");
    let target = request_line
        .split_whitespace()
        .nth(1)
        .expect("a request target")
        .to_owned();

    let mut headers = HashMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await.expect("a header line");
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
    }

    let length = headers
        .get("content-length")
        .and_then(|length| length.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).await.expect("the body");

    let (status, answer) = answering(&target);
    let answer = answer.to_string();
    let mut stream = reader.into_inner();
    stream
        .write_all(
            format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
                answer.len()
            )
            .as_bytes(),
        )
        .await
        .expect("the answer was sent");

    Asked {
        target,
        headers,
        body: String::from_utf8(body).expect("a text body"),
    }
}
