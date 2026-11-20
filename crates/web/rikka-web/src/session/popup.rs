use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Popup {
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl Popup {
    pub(crate) fn from_features(features: &str) -> Popup {
        let features = tokenize(features);
        Popup {
            width: features.get("width").and_then(|value| length(value)),
            height: features.get("height").and_then(|value| length(value)),
        }
    }
}

fn is_separator(c: char) -> bool {
    c.is_ascii_whitespace() || c == '=' || c == ','
}

fn normalized(name: String) -> String {
    match name.as_str() {
        "screenx" => "left".to_owned(),
        "screeny" => "top".to_owned(),
        "innerwidth" => "width".to_owned(),
        "innerheight" => "height".to_owned(),
        _ => name,
    }
}

fn tokenize(features: &str) -> HashMap<String, String> {
    let chars: Vec<char> = features.chars().collect();
    let mut tokens = HashMap::new();
    let mut at = 0;
    let collect = |at: &mut usize, keep: fn(char) -> bool| {
        let start = *at;
        while *at < chars.len() && keep(chars[*at]) {
            *at += 1;
        }
        chars[start..*at]
            .iter()
            .collect::<String>()
            .to_ascii_lowercase()
    };
    while at < chars.len() {
        collect(&mut at, is_separator);
        let name = normalized(collect(&mut at, |c| !is_separator(c)));
        while at < chars.len() && chars[at] != '=' {
            if chars[at] == ',' || !is_separator(chars[at]) {
                break;
            }
            at += 1;
        }
        let mut value = String::new();
        if at < chars.len() && is_separator(chars[at]) {
            while at < chars.len() && is_separator(chars[at]) && chars[at] != ',' {
                at += 1;
            }
            value = collect(&mut at, |c| !is_separator(c));
        }
        if !name.is_empty() {
            tokens.insert(name, value);
        }
    }
    tokens
}

fn length(value: &str) -> Option<u32> {
    let value = value.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let (negative, digits) = match value.as_bytes().first() {
        Some(b'-') => (true, &value[1..]),
        Some(b'+') => (false, &value[1..]),
        _ => (false, value),
    };
    let end = digits
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(digits.len());
    let parsed: u64 = digits[..end].parse().ok()?;
    match negative || parsed == 0 {
        true => None,
        false => Some(u32::try_from(parsed).unwrap_or(u32::MAX)),
    }
}
