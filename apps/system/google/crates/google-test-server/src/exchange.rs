use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    #[must_use]
    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or_default()
    }

    #[must_use]
    pub fn query(&self) -> HashMap<String, String> {
        let query = self.target.split_once('?').map_or("", |(_, query)| query);
        pairs(query)
    }

    #[must_use]
    pub fn form(&self) -> HashMap<String, String> {
        pairs(&String::from_utf8_lossy(&self.body))
    }

    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }

    #[must_use]
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub(crate) fn read(reader: &mut BufReader<TcpStream>) -> Option<Request> {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let mut parts = line.split_whitespace();
        let method = parts.next()?.to_owned();
        let target = parts.next()?.to_owned();
        let mut headers = HashMap::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).ok()?;
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            let (name, value) = line.split_once(':')?;
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
        let chunked = headers
            .get("transfer-encoding")
            .is_some_and(|coding| coding.eq_ignore_ascii_case("chunked"));
        let body = match chunked {
            true => chunks(reader)?,
            false => {
                let length = headers
                    .get("content-length")
                    .and_then(|length| length.parse().ok())
                    .unwrap_or(0);
                let mut body = vec![0; length];
                reader.read_exact(&mut body).ok()?;
                body
            }
        };
        Some(Request {
            method,
            target,
            headers,
            body,
        })
    }
}

impl Response {
    #[must_use]
    pub fn ok(body: impl Into<Vec<u8>>) -> Response {
        Response::status(200).body(body)
    }

    #[must_use]
    pub fn json(body: &str) -> Response {
        Response::ok(body).header("Content-Type", "application/json")
    }

    #[must_use]
    pub fn status(status: u16) -> Response {
        Response {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Response {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    #[must_use]
    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Response {
        self.body = body.into();
        self
    }

    pub(crate) fn write(&self, stream: &mut TcpStream) -> std::io::Result<()> {
        let mut head = Vec::new();
        write!(head, "HTTP/1.1 {} Answer\r\n", self.status)?;
        for (name, value) in &self.headers {
            write!(head, "{name}: {value}\r\n")?;
        }
        write!(head, "Content-Length: {}\r\n\r\n", self.body.len())?;
        stream.write_all(&head)?;
        stream.write_all(&self.body)?;
        stream.flush()
    }
}

fn chunks(reader: &mut BufReader<TcpStream>) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let size = usize::from_str_radix(line.trim().split(';').next()?, 16).ok()?;
        let mut chunk = vec![0; size + 2];
        reader.read_exact(&mut chunk).ok()?;
        if size == 0 {
            return Some(body);
        }
        body.extend_from_slice(&chunk[..size]);
    }
}

fn pairs(encoded: &str) -> HashMap<String, String> {
    encoded
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(name), decode(value))
        })
        .collect()
}

fn decode(encoded: &str) -> String {
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'+' => decoded.push(b' '),
            b'%' => {
                let value = bytes
                    .get(at + 1..at + 3)
                    .and_then(|digits| std::str::from_utf8(digits).ok())
                    .and_then(|digits| u8::from_str_radix(digits, 16).ok());
                match value {
                    Some(value) => {
                        decoded.push(value);
                        at += 2;
                    }
                    None => decoded.push(b'%'),
                }
            }
            byte => decoded.push(byte),
        }
        at += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}
