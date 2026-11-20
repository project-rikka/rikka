use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    #[must_use]
    pub fn route(&self) -> (&str, &str) {
        (&self.method, &self.path)
    }

    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    #[must_use]
    pub fn bearer(&self) -> &str {
        self.header("authorization")
            .map_or("", |value| value.trim_start_matches("Bearer "))
    }

    #[must_use]
    pub fn query(&self, name: &str) -> Option<&str> {
        self.query
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value)
    }

    #[must_use]
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }

    #[must_use]
    pub fn form(&self) -> Vec<(String, Vec<u8>)> {
        form_fields(&self.body)
    }

    #[must_use]
    pub fn field(&self, name: &str) -> Option<Vec<u8>> {
        self.form()
            .into_iter()
            .find(|(field_name, _)| field_name == name)
            .map(|(_, value)| value)
    }
}

fn form_fields(body: &[u8]) -> Vec<(String, Vec<u8>)> {
    let Some(first_break) = find(body, b"\r\n") else {
        return Vec::new();
    };
    let delimiter = [b"\r\n".as_slice(), &body[..first_break]].concat();
    let mut fields = Vec::new();
    let mut rest = &body[first_break + 2..];

    while let Some(head_end) = find(rest, b"\r\n\r\n") {
        let head = String::from_utf8_lossy(&rest[..head_end]).into_owned();
        let after_head = &rest[head_end + 4..];
        let Some(value_end) = find(after_head, &delimiter) else {
            break;
        };
        let name = head
            .split("name=\"")
            .nth(1)
            .and_then(|tail| tail.split('"').next())
            .unwrap_or_default()
            .to_owned();
        fields.push((name, after_head[..value_end].to_vec()));

        let next = &after_head[value_end + delimiter.len()..];
        if next.starts_with(b"--") {
            break;
        }
        rest = next.strip_prefix(b"\r\n").unwrap_or(next);
    }

    fields
}

pub(crate) fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
