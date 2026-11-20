use std::borrow::Cow;

use rikka_proton_contentline::{decode_charset, decode_quoted_printable};

const MARKER: &str = "QUOTED-PRINTABLE";

pub(crate) fn join_soft_line_breaks(text: &str) -> Cow<'_, str> {
    if !text.to_ascii_uppercase().contains(MARKER) {
        return Cow::Borrowed(text);
    }
    let mut joined = String::with_capacity(text.len());
    let mut continuing = false;
    for physical in text.split('\n') {
        let line = physical.strip_suffix('\r').unwrap_or(physical);
        let encoded = continuing
            || line
                .split(':')
                .next()
                .is_some_and(|head| head.to_ascii_uppercase().contains(MARKER));
        match line.strip_suffix('=') {
            Some(soft) if encoded => {
                joined.push_str(soft);
                continuing = true;
            }
            _ => {
                joined.push_str(line);
                joined.push_str("\r\n");
                continuing = false;
            }
        }
    }
    Cow::Owned(joined)
}

pub(crate) fn decode_quoted_printable_value(value: &str, charset: Option<&str>) -> String {
    decode_charset(&decode_quoted_printable(value.as_bytes()), charset)
}
