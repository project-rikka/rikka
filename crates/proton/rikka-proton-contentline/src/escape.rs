pub fn escape_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(character) = chars.next() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            ';' => escaped.push_str("\\;"),
            ',' => escaped.push_str("\\,"),
            '\n' => escaped.push_str("\\n"),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                escaped.push_str("\\n");
            }
            other => escaped.push(other),
        }
    }

    escaped
}

pub fn unescape_text(text: &str) -> String {
    let mut unescaped = String::with_capacity(text.len());
    let mut chars = text.chars();

    while let Some(character) = chars.next() {
        if character != '\\' {
            unescaped.push(character);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => unescaped.push('\n'),
            Some(literal @ ('\\' | ';' | ',' | ':' | '"')) => unescaped.push(literal),
            Some(other) => {
                unescaped.push('\\');
                unescaped.push(other);
            }
            None => unescaped.push('\\'),
        }
    }

    unescaped
}

pub fn split_unescaped(value: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut escaped = false;

    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
        } else if character == separator {
            parts.push(&value[start..index]);
            start = index + character.len_utf8();
        }
    }
    parts.push(&value[start..]);

    parts
}
