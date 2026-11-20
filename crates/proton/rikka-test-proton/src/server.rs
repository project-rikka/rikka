use std::fmt;
use std::io;
use std::sync::Arc;

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};

use crate::request::{Request, find};
use crate::response::{Response, ResponseFuture};

const CHUNK: usize = 64 * 1024;

pub async fn serve<H>(handler: H) -> String
where
    H: Fn(Request) -> ResponseFuture + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let handler = Arc::new(handler);

    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let handler = Arc::clone(&handler);
            tokio::spawn(async move { serve_connection(stream, handler).await.ok() });
        }
    });

    format!("http://{address}")
}

async fn serve_connection<H>(mut stream: TcpStream, handler: Arc<H>) -> io::Result<()>
where
    H: Fn(Request) -> ResponseFuture + Send + Sync + 'static,
{
    let Some(request) = read_request(&mut stream).await else {
        return Ok(());
    };
    let response = handler(request).await;

    stream
        .write_all(ResponseHead(&response).to_string().as_bytes())
        .await?;
    stream.write_all(&response.body).await?;
    stream.flush().await
}

struct ResponseHead<'a>(&'a Response);

impl fmt::Display for ResponseHead<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let response = self.0;
        write!(
            out,
            "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n",
            response.status,
            response.body.len()
        )?;
        let has_content_type = response
            .headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("content-type"));
        if !has_content_type {
            out.write_str("Content-Type: application/json\r\n")?;
        }
        for (name, value) in &response.headers {
            write!(out, "{name}: {value}\r\n")?;
        }
        out.write_str("\r\n")
    }
}

async fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut raw = Vec::new();
    let mut chunk = vec![0u8; CHUNK];

    let end = loop {
        if let Some(end) = find(&raw, b"\r\n\r\n") {
            break end + 4;
        }
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return None,
            Ok(read) => raw.extend_from_slice(&chunk[..read]),
        }
    };

    let text = String::from_utf8_lossy(&raw[..end]).into_owned();
    let mut lines = text.lines();
    let mut start = lines.next()?.split_whitespace();
    let method = start.next()?.to_owned();
    let target = start.next()?.to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_owned(), value.trim().to_owned()))
        .collect();
    let length: usize = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse().ok())
        .unwrap_or_default();

    let mut body = raw[end..].to_vec();
    while body.len() < length {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(read) => body.extend_from_slice(&chunk[..read]),
        }
    }
    let (path, query) = target.split_once('?').unwrap_or((target.as_str(), ""));

    Some(Request {
        method,
        path: path.to_owned(),
        query: query.to_owned(),
        headers,
        body,
    })
}
