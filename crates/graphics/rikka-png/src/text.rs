use rikka_deflate::Inflater;

use crate::chunk::{ITXT, TEXT, ZTXT};
use crate::error::ChunkKind;

pub(crate) const MAX_TEXT: usize = 4 << 20;
const MAX_KEYWORD: usize = 79;
const DEFLATE: u8 = 0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    pub keyword: String,
    pub text: String,
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&byte| char::from(byte)).collect()
}

fn is_latin1(text: &str) -> bool {
    text.chars().all(|character| u32::from(character) <= 0xFF)
}

pub(crate) fn decode(kind: ChunkKind, body: &[u8], inflater: &mut Inflater) -> Option<Text> {
    let end = body
        .iter()
        .take(MAX_KEYWORD + 1)
        .position(|&byte| byte == 0)?;
    let keyword = latin1(&body[..end]);
    let rest = &body[end + 1..];
    let text = match kind {
        TEXT => latin1(rest),
        ZTXT => {
            let (&method, compressed) = rest.split_first()?;
            if method != DEFLATE {
                return None;
            }
            let mut inflated = Vec::new();
            inflater.zlib(compressed, &mut inflated, MAX_TEXT).ok()?;
            latin1(&inflated)
        }
        ITXT => {
            let (&[compressed, method], rest) = rest.split_first_chunk::<2>()?;
            let language_end = rest.iter().position(|&byte| byte == 0)?;
            let rest = &rest[language_end + 1..];
            let translated_end = rest.iter().position(|&byte| byte == 0)?;
            let rest = &rest[translated_end + 1..];
            match compressed {
                0 => String::from_utf8(rest.to_vec()).ok()?,
                1 if method == DEFLATE => {
                    let mut inflated = Vec::new();
                    inflater.zlib(rest, &mut inflated, MAX_TEXT).ok()?;
                    String::from_utf8(inflated).ok()?
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    Some(Text { keyword, text })
}

pub(crate) fn encode(entry: &Text) -> Option<(ChunkKind, Vec<u8>)> {
    let keyword = entry.keyword.trim();
    let valid = (1..=MAX_KEYWORD).contains(&keyword.chars().count())
        && keyword
            .chars()
            .all(|character| matches!(u32::from(character), 0x20..=0x7E | 0xA1..=0xFF));
    if !valid {
        return None;
    }
    let mut body: Vec<u8> = keyword.chars().map(|character| character as u8).collect();
    body.push(0);
    if is_latin1(&entry.text) {
        body.extend(entry.text.chars().map(|character| character as u8));
        Some((TEXT, body))
    } else {
        body.extend_from_slice(&[0, 0, 0, 0]);
        body.extend_from_slice(entry.text.as_bytes());
        Some((ITXT, body))
    }
}
