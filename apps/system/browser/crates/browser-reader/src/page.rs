use rikka_web::ScriptAsk;
use serde_json::{Value, json};

pub const PAGE: &str = "browser-reader/reader.html";

pub const DOCUMENT: &str = "browser-reader.document";

const PART: usize = 128 * 1024;

pub fn serve(ask: ScriptAsk, document: &str) {
    let part = ask
        .body
        .get("index")
        .and_then(Value::as_u64)
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| part(document, index));
    let reply = match part {
        Some((text, more)) => json!({ "text": text, "more": more }),
        None => Value::Null,
    };
    ask.respond(reply);
}

fn part(document: &str, index: usize) -> Option<(&str, bool)> {
    let start = boundary(document, index.checked_mul(PART)?);
    let end = boundary(document, index.checked_add(1)?.checked_mul(PART)?);
    if start == document.len() && index > 0 {
        return None;
    }
    Some((&document[start..end], end < document.len()))
}

fn boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}
