use rikka_proton_contentline::decode_q_encoded;

use crate::mime::{decode_charset, transfer};

struct EncodedWord<'a> {
    charset: &'a str,
    bytes: Vec<u8>,
    encoded_len: usize,
}

pub(crate) fn decode(text: &str) -> String {
    if !text.contains("=?") {
        return text.to_owned();
    }

    let mut decoded = String::with_capacity(text.len());
    let mut pending: Option<(String, Vec<u8>)> = None;
    let mut gap = String::new();
    let mut rest = text;

    while !rest.is_empty() {
        if let Some(word) = parse_encoded_word(rest) {
            match &mut pending {
                Some((pending_charset, bytes))
                    if pending_charset.eq_ignore_ascii_case(word.charset) =>
                {
                    bytes.extend_from_slice(&word.bytes);
                }
                _ => {
                    flush(&mut decoded, pending.take());
                    pending = Some((word.charset.to_owned(), word.bytes));
                }
            }
            gap.clear();
            rest = &rest[word.encoded_len..];
            continue;
        }

        let Some(next) = rest.chars().next() else {
            break;
        };

        if pending.is_some() && (next == ' ' || next == '\t' || next == '\r' || next == '\n') {
            gap.push(next);
        } else {
            flush(&mut decoded, pending.take());
            decoded.push_str(&gap);
            gap.clear();
            decoded.push(next);
        }
        rest = &rest[next.len_utf8()..];
    }

    flush(&mut decoded, pending);
    decoded.push_str(&gap);
    decoded
}

fn flush(decoded: &mut String, pending: Option<(String, Vec<u8>)>) {
    if let Some((charset_label, bytes)) = pending {
        decoded.push_str(&decode_charset(&bytes, Some(&charset_label)));
    }
}

fn parse_encoded_word(text: &str) -> Option<EncodedWord<'_>> {
    let inner = text.strip_prefix("=?")?;
    let (charset_label, after_label) = inner.split_once('?')?;
    if charset_label.is_empty() || charset_label.contains(char::is_whitespace) {
        return None;
    }

    let mut chars = after_label.chars();
    let encoding = chars.next()?.to_ascii_uppercase();
    if !matches!(encoding, 'B' | 'Q') || chars.next()? != '?' {
        return None;
    }

    let payload_start = &after_label[2..];
    let end = payload_start.find("?=")?;
    let payload = &payload_start[..end];
    if payload.contains([' ', '\t', '\n']) {
        return None;
    }

    let bytes = match encoding {
        'B' => transfer::base64(payload.as_bytes()),
        _ => decode_q_encoded(payload.as_bytes()),
    };

    let charset = charset_label
        .split_once('*')
        .map_or(charset_label, |(name, _)| name);
    let encoded_len = 2 + charset_label.len() + 1 + 2 + end + 2;

    Some(EncodedWord {
        charset,
        bytes,
        encoded_len,
    })
}
