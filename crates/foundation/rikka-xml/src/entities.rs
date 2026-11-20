use std::borrow::Cow;

const LONGEST_REFERENCE: usize = 12;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Decoded<'a> {
    pub(crate) text: Cow<'a, str>,
    pub(crate) unresolved: Option<usize>,
}

pub(crate) fn decode(text: &str) -> Decoded<'_> {
    if !text.contains('&') {
        return Decoded {
            text: Cow::Borrowed(text),
            unresolved: None,
        };
    }
    let mut decoded = String::with_capacity(text.len());
    let mut unresolved = None;
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        decoded.push_str(&rest[..start]);
        rest = &rest[start..];
        match reference(rest) {
            Some((character, consumed)) => {
                decoded.push(character);
                rest = &rest[consumed..];
            }
            None => {
                unresolved.get_or_insert(text.len() - rest.len());
                decoded.push('&');
                rest = &rest[1..];
            }
        }
    }
    decoded.push_str(rest);
    Decoded {
        text: Cow::Owned(decoded),
        unresolved,
    }
}

fn reference(text: &str) -> Option<(char, usize)> {
    let window = &text.as_bytes()[..text.len().min(LONGEST_REFERENCE)];
    let end = window.iter().position(|&byte| byte == b';')?;
    let body = &text[1..end];
    let character = match body {
        "lt" => '<',
        "gt" => '>',
        "amp" => '&',
        "quot" => '"',
        "apos" => '\'',
        _ => {
            let number = body.strip_prefix('#')?;
            let value = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(value)?
        }
    };
    Some((character, end + 1))
}
