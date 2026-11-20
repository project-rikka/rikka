#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Line {
    Prompt { text: String, echo: bool },
    Info(String),
    Error(String),
    Success,
    Failure,
}

impl Line {
    pub(crate) fn parse(raw: &str) -> Option<Line> {
        let raw = raw.trim_end_matches(['\n', '\r']);
        for (marker, echo) in [
            ("PAM_PROMPT_ECHO_OFF ", false),
            ("PAM_PROMPT_ECHO_ON ", true),
        ] {
            if let Some(text) = raw.strip_prefix(marker) {
                return Some(Line::Prompt {
                    text: unescape(text),
                    echo,
                });
            }
        }
        if let Some(text) = raw.strip_prefix("PAM_TEXT_INFO ") {
            return Some(Line::Info(unescape(text)));
        }
        if let Some(text) = raw.strip_prefix("PAM_ERROR_MSG ") {
            return Some(Line::Error(unescape(text)));
        }
        if raw.starts_with("SUCCESS") {
            return Some(Line::Success);
        }
        if raw.starts_with("FAILURE") {
            return Some(Line::Failure);
        }
        None
    }
}

fn unescape(escaped: &str) -> String {
    let bytes = escaped.as_bytes();
    let mut text = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] != b'\\' {
            text.push(bytes[at]);
            at += 1;
            continue;
        }
        at += 1;
        let Some(&marker) = bytes.get(at) else {
            text.push(b'\\');
            break;
        };
        at += 1;
        match marker {
            b'b' => text.push(0x08),
            b'f' => text.push(0x0c),
            b'n' => text.push(b'\n'),
            b'r' => text.push(b'\r'),
            b't' => text.push(b'\t'),
            b'v' => text.push(0x0b),
            b'0'..=b'7' => {
                let mut byte = u32::from(marker - b'0');
                for _ in 0..2 {
                    match bytes.get(at) {
                        Some(digit @ b'0'..=b'7') => {
                            byte = byte * 8 + u32::from(digit - b'0');
                            at += 1;
                        }
                        _ => break,
                    }
                }
                text.push(byte as u8);
            }
            other => text.push(other),
        }
    }
    String::from_utf8_lossy(&text).into_owned()
}
