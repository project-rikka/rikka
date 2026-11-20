use crate::mime::entities;

const SKIPPED: &[&str] = &["script", "style", "head", "title", "template"];

const BLOCKS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "canvas",
    "caption",
    "dd",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "main",
    "nav",
    "noscript",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "tr",
    "ul",
    "video",
];

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

pub fn html_to_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len() / 2);
    let mut rest = html;

    while !rest.is_empty() {
        let Some(open) = rest.find('<') else {
            decode_into(&mut text, rest);
            break;
        };
        decode_into(&mut text, &rest[..open]);
        rest = &rest[open..];

        if let Some(comment) = rest.strip_prefix("<!--") {
            rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
            continue;
        }
        if rest.starts_with("<!") || rest.starts_with("<?") {
            rest = rest.find('>').map_or("", |end| &rest[end + 1..]);
            continue;
        }

        let Some(tag) = Tag::parse(rest) else {
            text.push('<');
            rest = &rest[1..];
            continue;
        };
        rest = &rest[tag.length..];

        if tag.closing {
            continue;
        }

        let block = BLOCKS.contains(&tag.name.as_str());
        let void = VOID.contains(&tag.name.as_str()) || tag.self_closing;

        if !void && (SKIPPED.contains(&tag.name.as_str()) || tag.hidden) {
            rest = skip_element(rest, &tag.name);
            continue;
        }

        if tag.name == "br" || block {
            text.push('\n');
        }
    }

    collapse_whitespace(&text)
}

struct Tag {
    name: String,
    closing: bool,
    self_closing: bool,
    hidden: bool,
    length: usize,
}

impl Tag {
    fn parse(input: &str) -> Option<Tag> {
        let inner = &input[1..];
        let (closing, inner) = match inner.strip_prefix('/') {
            Some(after) => (true, after),
            None => (false, inner),
        };
        let name_length = inner
            .find(|character: char| {
                !(character.is_ascii_alphanumeric() || character == '-' || character == ':')
            })
            .unwrap_or(inner.len());
        if name_length == 0 || !inner.starts_with(|character: char| character.is_ascii_alphabetic())
        {
            return None;
        }
        let name = inner[..name_length].to_ascii_lowercase();

        let attributes_start = input.len() - inner.len() + name_length;
        let end = tag_end(&input[attributes_start..])?;
        let attributes = &input[attributes_start..attributes_start + end];

        Some(Tag {
            name,
            closing,
            self_closing: attributes.trim_end().ends_with('/'),
            hidden: is_hidden(attributes),
            length: attributes_start + end + 1,
        })
    }
}

fn tag_end(attributes: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (offset, character) in attributes.char_indices() {
        match (quote, character) {
            (Some(open), _) if character == open => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(character),
            (None, '>') => return Some(offset),
            (None, _) => {}
        }
    }
    None
}

fn is_hidden(attributes: &str) -> bool {
    let lowered = attributes.to_ascii_lowercase();
    let compact: String = lowered
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    if compact.contains("display:none") || compact.contains("visibility:hidden") {
        return true;
    }
    lowered
        .split(|character: char| character.is_whitespace() || character == '/')
        .any(|word| word == "hidden" || word.starts_with("hidden="))
}

fn skip_element<'a>(rest: &'a str, name: &str) -> &'a str {
    let raw = matches!(name, "script" | "style" | "title" | "template");
    let mut depth = 1usize;
    let mut cursor = 0;

    while let Some(found) = rest[cursor..].find('<') {
        let offset = cursor + found;
        let tail = &rest[offset..];

        if let Some(after) = strip_tag_name(tail.strip_prefix("</"), name) {
            depth -= 1;
            if depth == 0 {
                let consumed = tail.len() - after.len();
                let close = after.find('>').map_or(after.len(), |end| end + 1);
                return &rest[offset + consumed + close..];
            }
        } else if !raw && strip_tag_name(tail.strip_prefix('<'), name).is_some() {
            depth += 1;
        }

        cursor = offset + 1;
    }

    ""
}

fn strip_tag_name<'a>(after: Option<&'a str>, name: &str) -> Option<&'a str> {
    let after = after?;
    let head = after.get(..name.len())?;
    if !head.eq_ignore_ascii_case(name) {
        return None;
    }
    let tail = &after[name.len()..];
    tail.starts_with(|character: char| !(character.is_ascii_alphanumeric() || character == '-'))
        .then_some(tail)
        .or_else(|| tail.is_empty().then_some(tail))
}

fn decode_into(decoded: &mut String, text: &str) {
    let mut rest = text;

    while let Some(offset) = rest.find('&') {
        decoded.push_str(&rest[..offset]);
        rest = &rest[offset..];

        match char_reference(rest) {
            Some((character, length)) => {
                decoded.push(character);
                rest = &rest[length..];
            }
            None => {
                decoded.push('&');
                rest = &rest[1..];
            }
        }
    }

    decoded.push_str(rest);
}

fn char_reference(input: &str) -> Option<(char, usize)> {
    let body = &input[1..];

    if let Some(numeric) = body.strip_prefix('#') {
        let (digits, radix, prefix) = match numeric.strip_prefix(['x', 'X']) {
            Some(hex) => (hex, 16, 2),
            None => (numeric, 10, 1),
        };
        let length = digits
            .find(|character: char| !character.is_digit(radix))
            .unwrap_or(digits.len());
        if length == 0 {
            return None;
        }
        let point = u32::from_str_radix(&digits[..length], radix).ok()?;
        let character = match point {
            0 => '\u{FFFD}',
            point => char::from_u32(point).unwrap_or('\u{FFFD}'),
        };
        let terminated = usize::from(digits[length..].starts_with(';'));
        return Some((character, 1 + prefix + length + terminated));
    }

    let length = body
        .find(|character: char| !character.is_ascii_alphanumeric())
        .unwrap_or(body.len());
    if length == 0 || !body[length..].starts_with(';') {
        return None;
    }
    let character = entities::lookup_named(&body[..length])?;
    Some((character, 1 + length + 1))
}

fn collapse_whitespace(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut after_newline = false;
    let mut pending_space = false;

    for character in text.chars() {
        if character == '\n' {
            if !after_newline {
                output.push('\n');
            }
            after_newline = true;
            pending_space = false;
            continue;
        }

        if after_newline && character.is_whitespace() {
            continue;
        }

        if character == ' ' || character == '\t' {
            pending_space = true;
            continue;
        }

        if pending_space {
            output.push(' ');
            pending_space = false;
        }
        after_newline = false;
        output.push(character);
    }

    output.trim().to_owned()
}
