use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc;

use serde_json::Value;

pub(crate) enum Reply {
    Page {
        status: &'static str,
        headers: Vec<(&'static str, String)>,
        body: Option<Value>,
    },
    Events(Vec<String>),
    Listening(mpsc::Receiver<String>),
}

impl Reply {
    pub(crate) fn status(status: &'static str) -> Reply {
        Reply::Page {
            status,
            headers: Vec::new(),
            body: None,
        }
    }

    pub(crate) fn json(status: &'static str, body: Value) -> Reply {
        Reply::Page {
            status,
            headers: Vec::new(),
            body: Some(body),
        }
    }

    pub(crate) fn with_header(mut self, name: &'static str, value: String) -> Reply {
        if let Reply::Page { headers, .. } = &mut self {
            headers.push((name, value));
        }
        self
    }

    pub(crate) fn write(self, stream: &mut TcpStream) -> std::io::Result<()> {
        match self {
            Reply::Page {
                status,
                headers,
                body,
            } => {
                let body = body.map(|body| body.to_string()).unwrap_or_default();
                let mut head = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
                    body.len()
                );
                if !body.is_empty() {
                    head.push_str("Content-Type: application/json\r\n");
                }
                for (name, value) in headers {
                    head.push_str(&format!("{name}: {value}\r\n"));
                }
                stream.write_all(format!("{head}\r\n{body}").as_bytes())
            }
            Reply::Events(events) => {
                stream.write_all(EVENT_STREAM.as_bytes())?;
                for event in events {
                    stream.write_all(event.as_bytes())?;
                }
                stream.flush()
            }
            Reply::Listening(events) => {
                stream.write_all(EVENT_STREAM.as_bytes())?;
                stream.flush()?;
                for event in events {
                    stream.write_all(event.as_bytes())?;
                    stream.flush()?;
                }
                Ok(())
            }
        }
    }
}

const EVENT_STREAM: &str =
    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";

pub(crate) fn event(id: &str, message: &Value) -> String {
    format!("id: {id}\nevent: message\ndata: {message}\n\n")
}
