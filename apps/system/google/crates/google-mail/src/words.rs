use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rikka_encoding::Encoding;

const OPEN: &str = "=?";
const CLOSE: &str = "?=";
const LINE: usize = 76;
const WRAPPING: usize = "=?UTF-8?B??=".len();
const PHRASE_LEAD: usize = "From: ".len();
const FOLD: &str = "\r\n ";

pub(crate) fn decode(header: &str) -> String {
    let mut decoded = String::with_capacity(header.len());
    let mut rest = header;
    let mut after_word = false;
    while let Some(start) = rest.find(OPEN) {
        let (before, candidate) = rest.split_at(start);
        match word(candidate) {
            Some((text, length)) => {
                if !(after_word && before.chars().all(char::is_whitespace)) {
                    decoded.push_str(before);
                }
                decoded.push_str(&text);
                rest = &candidate[length..];
                after_word = true;
            }
            None => {
                decoded.push_str(before);
                decoded.push_str(OPEN);
                rest = &candidate[OPEN.len()..];
                after_word = false;
            }
        }
    }
    decoded.push_str(rest);
    decoded
}

pub(crate) fn phrase(name: &str) -> String {
    if !name.is_ascii() {
        return encoded(name, PHRASE_LEAD);
    }
    let plain = name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b" !#$%&'*+-/=?^_`{|}~".contains(&byte));
    match plain {
        true => name.to_owned(),
        false => {
            let mut quoted = String::with_capacity(name.len() + 2);
            quoted.push('"');
            for letter in name.chars().filter(|letter| !letter.is_control()) {
                if matches!(letter, '"' | '\\') {
                    quoted.push('\\');
                }
                quoted.push(letter);
            }
            quoted.push('"');
            quoted
        }
    }
}

pub(crate) fn unstructured(text: &str, lead: usize) -> String {
    let text: String = text
        .chars()
        .map(|letter| match letter.is_control() {
            true => ' ',
            false => letter,
        })
        .collect();
    match text.is_ascii() {
        true => text,
        false => encoded(&text, lead),
    }
}

fn encoded(text: &str, lead: usize) -> String {
    let room = |lead: usize| (LINE - WRAPPING - lead) / 4 * 3;
    let mut words = Vec::new();
    let mut chunk = String::new();
    let mut limit = room(lead);
    for letter in text.chars() {
        if chunk.len() + letter.len_utf8() > limit {
            words.push(std::mem::take(&mut chunk));
            limit = room(FOLD.len() - 2);
        }
        chunk.push(letter);
    }
    words.push(chunk);
    words
        .iter()
        .map(|chunk| format!("=?UTF-8?B?{}?=", STANDARD.encode(chunk)))
        .collect::<Vec<_>>()
        .join(FOLD)
}

fn word(candidate: &str) -> Option<(String, usize)> {
    let inner = candidate.strip_prefix(OPEN)?;
    let (charset, inner) = inner.split_once('?')?;
    let (coding, inner) = inner.split_once('?')?;
    let end = inner.find(CLOSE)?;
    let payload = &inner[..end];
    if charset.is_empty() || payload.contains(char::is_whitespace) {
        return None;
    }
    let bytes = match coding {
        "B" | "b" => STANDARD
            .decode(payload.trim_end_matches('='))
            .or_else(|_| STANDARD.decode(payload))
            .or_else(|_| {
                base64::engine::general_purpose::STANDARD_NO_PAD
                    .decode(payload.trim_end_matches('='))
            })
            .ok()?,
        "Q" | "q" => quoted(payload),
        _ => return None,
    };
    let charset = charset.split('*').next().unwrap_or(charset);
    let text = match Encoding::for_label(charset.as_bytes()) {
        Some(encoding) => encoding.decode_without_bom_handling(&bytes).0.into_owned(),
        None => String::from_utf8_lossy(&bytes).into_owned(),
    };
    let length = OPEN.len() + charset_length(candidate)? + coding.len() + 2 + end + CLOSE.len();
    Some((text, length))
}

fn charset_length(candidate: &str) -> Option<usize> {
    candidate.strip_prefix(OPEN)?.find('?')
}

fn quoted(payload: &str) -> Vec<u8> {
    let bytes = payload.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'_' => decoded.push(b' '),
            b'=' => {
                let value = bytes
                    .get(at + 1..at + 3)
                    .and_then(|digits| std::str::from_utf8(digits).ok())
                    .and_then(|digits| u8::from_str_radix(digits, 16).ok());
                match value {
                    Some(value) => {
                        decoded.push(value);
                        at += 2;
                    }
                    None => decoded.push(b'='),
                }
            }
            byte => decoded.push(byte),
        }
        at += 1;
    }
    decoded
}
