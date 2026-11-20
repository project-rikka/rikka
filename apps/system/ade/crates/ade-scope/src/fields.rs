use serde_json::{Map, Value};

const BLOCK: [char; 2] = ['|', '>'];

pub(crate) fn parse(front: &str) -> Map<String, Value> {
    let lines: Vec<Line<'_>> = front.lines().map(Line::new).collect();
    let mut fields = Map::new();
    let mut at = 0;

    while at < lines.len() {
        let line = &lines[at];
        at += 1;
        if line.indent > 0 || line.is_blank() {
            continue;
        }
        let Some((key, written)) = keyed(line.text) else {
            continue;
        };

        let nested = lines[at..]
            .iter()
            .take_while(|line| line.is_blank() || line.indent > 0 || line.is_item())
            .count();
        let under = &lines[at..at + nested];
        at += nested;
        fields.insert(key.to_owned(), value(written, under));
    }

    fields
}

#[derive(Clone, Copy)]
struct Line<'a> {
    indent: usize,
    text: &'a str,
}

impl<'a> Line<'a> {
    fn new(line: &'a str) -> Line<'a> {
        let text = line.trim_start_matches([' ', '\t']);
        Line {
            indent: line.len() - text.len(),
            text: text.trim_end(),
        }
    }

    fn is_blank(&self) -> bool {
        self.text.is_empty() || self.text.starts_with('#')
    }

    fn is_item(&self) -> bool {
        self.text == "-" || self.text.starts_with("- ")
    }
}

fn keyed(text: &str) -> Option<(&str, &str)> {
    let mut from = 0;
    loop {
        let colon = from + text[from..].find(':')?;
        let after = &text[colon + 1..];
        if after.is_empty() || after.starts_with([' ', '\t']) {
            let key = unquoted(text[..colon].trim());
            return (!key.is_empty()).then_some((key, after.trim()));
        }
        from = colon + 1;
    }
}

fn unquoted(key: &str) -> &str {
    key.strip_prefix('"')
        .and_then(|key| key.strip_suffix('"'))
        .or_else(|| key.strip_prefix('\'')?.strip_suffix('\''))
        .unwrap_or(key)
}

fn value(written: &str, under: &[Line<'_>]) -> Value {
    if written.starts_with(BLOCK) {
        return Value::String(block(written.starts_with('>'), under));
    }
    if written.starts_with('[') {
        return flow(&joined(written, under), ']')
            .map(|items| Value::Array(items.into_iter().map(scalar).collect()))
            .unwrap_or(Value::Null);
    }
    if written.starts_with('{') {
        return flow(&joined(written, under), '}')
            .map(|pairs| Value::Object(pairs.iter().filter_map(|pair| entry(pair)).collect()))
            .unwrap_or(Value::Null);
    }
    if !plain(written).is_empty() {
        return scalar(joined(written, under));
    }

    let written: Vec<&Line<'_>> = under.iter().filter(|line| !line.is_blank()).collect();
    match written.first() {
        Some(first) if first.is_item() => Value::Array(
            written
                .iter()
                .filter(|line| line.indent == first.indent && line.is_item())
                .map(|line| scalar(line.text[1..].trim().to_owned()))
                .collect(),
        ),
        Some(first) => Value::Object(
            written
                .iter()
                .filter(|line| line.indent == first.indent)
                .filter_map(|line| entry(line.text))
                .collect(),
        ),
        None => Value::Null,
    }
}

fn entry(pair: &str) -> Option<(String, Value)> {
    let (key, written) = keyed(pair.trim())?;
    Some((key.to_owned(), scalar(written.to_owned())))
}

fn joined(written: &str, under: &[Line<'_>]) -> String {
    let mut joined = written.to_owned();
    for line in under.iter().filter(|line| !line.is_blank()) {
        joined.push(' ');
        joined.push_str(line.text);
    }
    joined
}

fn block(folded: bool, under: &[Line<'_>]) -> String {
    let under: Vec<&Line<'_>> = under
        .iter()
        .filter(|line| line.indent > 0 || !line.text.starts_with('#'))
        .collect();
    let indent = under
        .iter()
        .filter(|line| !line.text.is_empty())
        .map(|line| line.indent)
        .min()
        .unwrap_or(0);

    let mut text = String::new();
    let mut broken = true;
    for line in under {
        if line.text.is_empty() {
            text.push('\n');
            broken = true;
            continue;
        }
        if !broken {
            text.push(match folded {
                true => ' ',
                false => '\n',
            });
        }
        text.extend(std::iter::repeat_n(' ', line.indent - indent));
        text.push_str(line.text);
        broken = false;
    }
    text.trim_end_matches('\n').to_owned()
}

fn flow(written: &str, closing: char) -> Option<Vec<String>> {
    let inside = written[1..].trim_end();
    let inside = inside[..inside.rfind(closing)?].trim();

    let mut items = Vec::new();
    let mut item = String::new();
    let mut quote = None;
    for letter in inside.chars() {
        match (quote, letter) {
            (None, ',') => items.push(std::mem::take(&mut item)),
            (None, '"' | '\'') => {
                quote = Some(letter);
                item.push(letter);
            }
            (Some(open), _) if open == letter => {
                quote = None;
                item.push(letter);
            }
            _ => item.push(letter),
        }
    }
    items.push(item);
    Some(
        items
            .into_iter()
            .map(|item| item.trim().to_owned())
            .filter(|item| !item.is_empty())
            .collect(),
    )
}

fn scalar(written: String) -> Value {
    let written = written.trim();
    if let Some(quoted) = written.strip_prefix('"') {
        return Value::String(escaped(quoted));
    }
    if let Some(quoted) = written.strip_prefix('\'') {
        let end = closing(quoted).unwrap_or(quoted.len());
        return Value::String(quoted[..end].replace("''", "'"));
    }
    match plain(written) {
        "" | "~" | "null" | "Null" | "NULL" => Value::Null,
        text => Value::String(text.to_owned()),
    }
}

fn plain(written: &str) -> &str {
    if written.starts_with('#') {
        return "";
    }
    let end = written
        .match_indices('#')
        .find(|(at, _)| written[..*at].ends_with([' ', '\t']))
        .map_or(written.len(), |(at, _)| at);
    written[..end].trim_end()
}

fn closing(quoted: &str) -> Option<usize> {
    let mut from = 0;
    loop {
        let quote = from + quoted[from..].find('\'')?;
        if !quoted[quote + 1..].starts_with('\'') {
            return Some(quote);
        }
        from = quote + 2;
    }
}

fn escaped(quoted: &str) -> String {
    let mut text = String::with_capacity(quoted.len());
    let mut letters = quoted.chars();
    while let Some(letter) = letters.next() {
        match letter {
            '"' => break,
            '\\' => match letters.next() {
                Some('n') => text.push('\n'),
                Some('t') => text.push('\t'),
                Some('r') => text.push('\r'),
                Some('0') => text.push('\0'),
                Some('u') => text.extend(coded(&mut letters, 4)),
                Some('U') => text.extend(coded(&mut letters, 8)),
                Some('x') => text.extend(coded(&mut letters, 2)),
                Some(other) => text.push(other),
                None => break,
            },
            other => text.push(other),
        }
    }
    text
}

fn coded(letters: &mut std::str::Chars<'_>, digits: usize) -> Option<char> {
    let code: String = letters.take(digits).collect();
    char::from_u32(u32::from_str_radix(&code, 16).ok()?)
}
