use rikka_sync::Mutex;
use std::collections::HashMap;
use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use rikka_certificate_transparency::{LogListKey, LogListSource, LogLists};

use crate::list::log_list_at;
use crate::signer::Signer;

const LIST_PATH: &str = "/log_list.json";
const SIGNATURE_PATH: &str = "/log_list.sig";

#[derive(Debug, Clone)]
pub struct LogListServer {
    address: String,
    files: Arc<Mutex<HashMap<&'static str, Vec<u8>>>>,
    requests: Arc<AtomicUsize>,
}

impl LogListServer {
    #[must_use]
    pub fn start() -> LogListServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("binding a local port should work");
        let address = listener
            .local_addr()
            .expect("a bound listener has an address");
        let server = LogListServer {
            address: format!("http://{address}"),
            files: Arc::default(),
            requests: Arc::default(),
        };
        let serving = server.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let serving = serving.clone();
                thread::spawn(move || serving.answer(stream));
            }
        });
        server
    }

    pub fn publish(&self, list: &[u8], signature: &[u8]) {
        let mut files = self.files.lock();
        files.insert(LIST_PATH, list.to_vec());
        files.insert(SIGNATURE_PATH, signature.to_vec());
    }

    #[must_use]
    pub fn source(&self, key: LogListKey) -> LogListSource {
        LogListSource::new(
            &format!("{}{LIST_PATH}", self.address),
            &format!("{}{SIGNATURE_PATH}", self.address),
            key,
        )
        .expect("a local server address is a valid URL")
    }

    #[must_use]
    pub fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }

    fn answer(&self, mut stream: TcpStream) {
        let mut request = Vec::new();
        let mut chunk = [0u8; 4096];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => return,
                Ok(read) => request.extend_from_slice(&chunk[..read]),
            }
        }
        self.requests.fetch_add(1, Ordering::SeqCst);
        let request = String::from_utf8_lossy(&request);
        let path = request.split_whitespace().nth(1).unwrap_or_default();
        let body = self.files.lock().get(path).cloned();
        let (status, body) = match body {
            Some(body) => ("200 OK", body),
            None => ("404 Not Found", Vec::new()),
        };
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let written = stream
            .write_all(head.as_bytes())
            .and_then(|()| stream.write_all(&body))
            .and_then(|()| stream.flush());
        if let Err(error) = written {
            eprintln!("log list server failed to answer: {error}");
        }
    }
}

#[must_use]
pub fn serving(timestamp: &str) -> LogLists {
    let server = LogListServer::start();
    let signer = Signer::new();
    let list = log_list_at(timestamp);
    server.publish(&list, &signer.sign(&list));
    LogLists::builder()
        .source(server.source(signer.key()))
        .build()
}
