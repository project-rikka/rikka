use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpStream;

use rikka_net::Url;
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Asked {
    method: String,
    path: String,
    query: String,
    headers: HashMap<String, String>,
    body: String,
}

impl Asked {
    pub(crate) fn read(stream: &TcpStream) -> Option<Asked> {
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).ok()?;
        let mut words = request_line.split_whitespace();
        let method = words.next()?.to_owned();
        let target = words.next()?;
        let (path, query) = target.split_once('?').unwrap_or((target, ""));

        let mut headers = HashMap::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).ok()?;
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
        reader.read_exact(&mut body).ok()?;

        Some(Asked {
            method,
            path: path.to_owned(),
            query: query.to_owned(),
            headers,
            body: String::from_utf8(body).ok()?,
        })
    }

    #[must_use]
    pub fn method(&self) -> &str {
        &self.method
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }

    #[must_use]
    pub fn json(&self) -> Option<Value> {
        serde_json::from_str(&self.body).ok()
    }

    #[must_use]
    pub fn rpc_method(&self) -> Option<String> {
        Some(self.json()?.get("method")?.as_str()?.to_owned())
    }

    #[must_use]
    pub fn field(&self, name: &str) -> Option<String> {
        [&self.query, &self.body].into_iter().find_map(|encoded| {
            Url::parse(&format!("http://localhost/?{encoded}"))
                .ok()?
                .query_pairs()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.into_owned())
        })
    }
}
