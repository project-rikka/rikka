use encoding_rs::{Encoding, WINDOWS_1252};

#[must_use]
pub fn decode_charset(bytes: &[u8], label: Option<&str>) -> String {
    match label.and_then(encoding_for_label) {
        Some(encoding) => encoding.decode(bytes).0.into_owned(),
        None => decode_unlabelled(bytes),
    }
}

fn encoding_for_label(label: &str) -> Option<&'static Encoding> {
    let trimmed = label.trim().trim_matches('"');
    Encoding::for_label(trimmed.as_bytes()).or_else(|| {
        trimmed
            .strip_prefix("cs")
            .and_then(|stripped| Encoding::for_label(stripped.as_bytes()))
    })
}

fn decode_unlabelled(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => WINDOWS_1252.decode(bytes).0.into_owned(),
    }
}
