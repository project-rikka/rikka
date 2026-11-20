use std::fmt::Write as _;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug, Clone)]
pub(crate) struct Request {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

impl Request {
    #[must_use]
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    #[must_use]
    pub(crate) fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

pub(crate) async fn spawn<F>(handler: F) -> String
where
    F: Fn(Request) -> Option<Vec<u8>> + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a port is free");
    let address: SocketAddr = listener.local_addr().expect("the socket has an address");
    let handler = Arc::new(handler);

    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let handler = Arc::clone(&handler);
            tokio::spawn(serve(stream, handler));
        }
    });

    format!("http://{address}")
}

async fn serve<F>(mut stream: TcpStream, handler: Arc<F>)
where
    F: Fn(Request) -> Option<Vec<u8>> + Send + Sync + 'static,
{
    let Some(request) = read_request(&mut stream).await else {
        return;
    };
    let Some(response) = handler(request) else {
        return;
    };

    let _ = stream.write_all(&response).await;
    let _ = stream.flush().await;
}

async fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];

    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte).await {
            Ok(0) | Err(_) => return None,
            Ok(_) => head.push(byte[0]),
        }
    }

    let text = String::from_utf8_lossy(&head).into_owned();
    let mut lines = text.lines();

    let mut start = lines.next()?.split_whitespace();
    let method = start.next()?.to_owned();
    let path = start.next()?.to_owned();

    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_owned(), value.trim().to_owned()))
        .collect();

    let length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);

    let mut body = vec![0u8; length];
    if length > 0 && stream.read_exact(&mut body).await.is_err() {
        return None;
    }

    Some(Request {
        method,
        path,
        headers,
        body,
    })
}

pub(crate) async fn spawn_echo() -> String {
    spawn_websocket(|mut socket| async move {
        use futures_util::{SinkExt, StreamExt};

        while let Some(Ok(message)) = socket.next().await {
            if message.is_close() {
                break;
            }
            if message.is_text() || message.is_binary() {
                let _ = socket.send(message).await;
            }
        }
    })
    .await
}

pub(crate) async fn spawn_dropping() -> (String, Arc<std::sync::atomic::AtomicU32>) {
    use std::sync::atomic::{AtomicU32, Ordering};

    let connections = Arc::new(AtomicU32::new(0));
    let counter = Arc::clone(&connections);

    let url = spawn_websocket(move |mut socket| {
        let counter = Arc::clone(&counter);
        async move {
            use futures_util::SinkExt;
            use tokio_tungstenite::tungstenite::Message;

            let count = counter.fetch_add(1, Ordering::SeqCst) + 1;
            let _ = socket
                .send(Message::text(format!("connection {count}")))
                .await;
        }
    })
    .await;

    (url, connections)
}

async fn spawn_websocket<F, Fut>(handler: F) -> String
where
    F: Fn(tokio_tungstenite::WebSocketStream<TcpStream>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a port is free");
    let address: SocketAddr = listener.local_addr().expect("the socket has an address");
    let handler = Arc::new(handler);

    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let handler = Arc::clone(&handler);
            tokio::spawn(async move {
                let Ok(socket) = tokio_tungstenite::accept_async(stream).await else {
                    return;
                };
                handler(socket).await;
            });
        }
    });

    format!("ws://{address}")
}

#[must_use]
pub(crate) fn response(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    truncated(status, headers, body, body.len())
}

#[must_use]
pub(crate) fn truncated(
    status: u16,
    headers: &[(&str, &str)],
    body: &[u8],
    declared: usize,
) -> Vec<u8> {
    let mut head = format!(
        "HTTP/1.1 {status} {}\r\nContent-Length: {declared}\r\nConnection: close\r\n",
        reason(status)
    );
    for (name, value) in headers {
        writeln!(head, "{name}: {value}\r").expect("formatting into a String is infallible");
    }
    head.push_str("\r\n");

    let mut response = head.into_bytes();
    response.extend_from_slice(body);
    response
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        206 => "Partial Content",
        304 => "Not Modified",
        400 => "Bad Request",
        404 => "Not Found",
        416 => "Range Not Satisfiable",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}
