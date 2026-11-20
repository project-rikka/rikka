#[must_use]
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    let starts = |magic: &[u8]| bytes.starts_with(magic);
    if starts(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if starts(b"\xFF\xD8\xFF") {
        return Some("image/jpeg");
    }
    if starts(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Some("image/webp");
    }
    if starts(b"\xABKTX 20\xBB\r\n\x1A\n") {
        return Some("image/ktx2");
    }
    None
}

#[must_use]
pub fn guess(bytes: &[u8], name: &str) -> &'static str {
    if let Some(sniffed) = sniff(bytes) {
        return sniffed;
    }
    match name
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ktx2" => "image/ktx2",
        _ => "image/png",
    }
}

#[must_use]
pub fn extension(mime: &str) -> &'static str {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/ktx2" => "ktx2",
        _ => "png",
    }
}
