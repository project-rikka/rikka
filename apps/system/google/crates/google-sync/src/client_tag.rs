use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sha1::{Digest, Sha1};

use crate::proto::PasswordSpecificsData;

const PASSWORDS_PREFIX: [u8; 4] = [0x8a, 0xb3, 0x16, 0x00];
const HEX: &[u8; 16] = b"0123456789ABCDEF";

#[must_use]
pub fn client_tag(data: &PasswordSpecificsData) -> String {
    let field = |value: &Option<String>| escape_path(value.as_deref().unwrap_or_default());
    let origin = data
        .origin
        .as_deref()
        .and_then(|origin| rikka_net::Url::parse(origin).ok())
        .map(String::from)
        .unwrap_or_default();
    format!(
        "{}|{}|{}|{}|{}",
        escape_path(&origin),
        field(&data.username_element),
        field(&data.username_value),
        field(&data.password_element),
        field(&data.signon_realm),
    )
}

#[must_use]
pub fn client_tag_hash(data: &PasswordSpecificsData) -> String {
    let mut hasher = Sha1::new();
    hasher.update(PASSWORDS_PREFIX);
    hasher.update(client_tag(data).as_bytes());
    STANDARD.encode(hasher.finalize())
}

fn escape_path(path: &str) -> String {
    let mut escaped = String::with_capacity(path.len());
    for byte in path.bytes() {
        let kept = byte.is_ascii_graphic()
            && !matches!(
                byte,
                b'"' | b'#'
                    | b'%'
                    | b':'
                    | b'<'
                    | b'>'
                    | b'?'
                    | b'['
                    | b'\\'
                    | b']'
                    | b'^'
                    | b'`'
                    | b'{'
                    | b'|'
                    | b'}'
            );
        match kept {
            true => escaped.push(char::from(byte)),
            false => {
                escaped.push('%');
                escaped.push(char::from(HEX[usize::from(byte >> 4)]));
                escaped.push(char::from(HEX[usize::from(byte & 0x0f)]));
            }
        }
    }
    escaped
}
