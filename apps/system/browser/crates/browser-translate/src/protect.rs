use std::ops::Range;

const MARKERS: &[&str] = &["TODO:", "FIXME:", "NOTE:", "XXX:", "HACK:", "SAFETY:"];
const MARKER_LEADERS: &[char] = &['/', '#', '*', '-', ';', '!', '<', '(', '{', '%'];
const OPENERS: &[u8] = b"([{\"'<";
const TRAILERS: &[u8] = b".,;:!?\"'";
const ABBREVIATIONS: &[&str] = &[
    "a.d", "a.k.a", "a.m", "b.c", "d.c", "e.g", "e.u", "i.e", "n.b", "p.m", "p.s", "ph.d", "q.v",
    "r.i.p", "u.k", "u.n", "u.s", "u.s.a",
];
const SCHEMES: &[&str] = &[
    "http://", "https://", "ftp://", "file://", "ws://", "wss://",
];

pub(crate) fn protected(text: &str) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut from = 0;
    for span in backtick_spans(text) {
        scan_words(text, from..span.start, &mut spans);
        spans.push(span.clone());
        from = span.end;
    }
    scan_words(text, from..text.len(), &mut spans);
    spans
}

fn backtick_spans(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] != b'`' {
            at += 1;
            continue;
        }
        let fence = run_length(bytes, at, b'`');
        let mut search = at + fence;
        let mut closed = None;
        while search < bytes.len() {
            if bytes[search] == b'`' {
                let length = run_length(bytes, search, b'`');
                if length == fence {
                    closed = Some(search + length);
                    break;
                }
                search += length;
            } else {
                search += 1;
            }
        }
        match closed {
            Some(end) if end > at + 2 * fence => {
                spans.push(at..end);
                at = end;
            }
            _ => at += fence,
        }
    }
    spans
}

fn run_length(bytes: &[u8], from: usize, byte: u8) -> usize {
    bytes[from..].iter().take_while(|b| **b == byte).count()
}

fn scan_words(text: &str, region: Range<usize>, found: &mut Vec<Range<usize>>) {
    let bytes = text.as_bytes();
    let mut at = region.start;
    while at < region.end {
        if !bytes[at].is_ascii_graphic() {
            at += 1;
            continue;
        }
        let start = at;
        while at < region.end && bytes[at].is_ascii_graphic() {
            at += 1;
        }
        if let Some(token) = token_in(text, start..at) {
            found.push(token);
        }
    }
}

fn token_in(text: &str, run: Range<usize>) -> Option<Range<usize>> {
    let word = &text[run.clone()];
    if MARKERS.contains(&word) && leads_line(text, run.start) {
        return Some(run);
    }
    let core = trim(word, run.start);
    let piece = &text[core.clone()];
    (!piece.is_empty() && is_code(piece)).then_some(core)
}

fn leads_line(text: &str, start: usize) -> bool {
    text[..start]
        .chars()
        .rev()
        .take_while(|c| *c != '\n')
        .all(|c| c.is_whitespace() || MARKER_LEADERS.contains(&c))
}

fn trim(word: &str, offset: usize) -> Range<usize> {
    let bytes = word.as_bytes();
    let mut start = 0;
    let mut end = bytes.len();
    while start < end && OPENERS.contains(&bytes[start]) {
        start += 1;
    }
    loop {
        if end <= start {
            break;
        }
        let last = bytes[end - 1];
        if end - start >= 3 && bytes[end - 2] == b'\'' && last == b's' {
            end -= 2;
        } else if TRAILERS.contains(&last) {
            end -= 1;
        } else if let Some(open) = opener_of(last) {
            let slice = &bytes[start..end];
            let opened = slice.iter().filter(|b| **b == open).count();
            let closed = slice.iter().filter(|b| **b == last).count();
            if closed > opened {
                end -= 1;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    offset + start..offset + end
}

fn opener_of(byte: u8) -> Option<u8> {
    match byte {
        b')' => Some(b'('),
        b']' => Some(b'['),
        b'}' => Some(b'{'),
        b'>' => Some(b'<'),
        _ => None,
    }
}

fn is_code(word: &str) -> bool {
    is_url(word)
        || is_email(word)
        || is_flag(word)
        || is_mention(word)
        || is_issue(word)
        || is_variable(word)
        || is_path(word)
        || is_version(word)
        || is_hash(word)
        || is_expression(word)
}

fn is_url(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    SCHEMES
        .iter()
        .any(|scheme| lower.starts_with(scheme) && lower.len() > scheme.len())
        || (lower.starts_with("www.") && lower[4..].contains('.'))
        || (lower.starts_with("mailto:") && is_email(&word[7..]))
}

fn is_email(word: &str) -> bool {
    let Some((local, domain)) = word.split_once('@') else {
        return false;
    };
    let tld = domain.rsplit('.').next().unwrap_or_default();
    !local.is_empty()
        && local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._%+-".contains(&b))
        && domain.contains('.')
        && domain
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        && domain.split('.').all(|label| !label.is_empty())
        && tld.len() >= 2
        && tld.bytes().all(|b| b.is_ascii_alphabetic())
}

fn is_flag(word: &str) -> bool {
    word.strip_prefix("--").is_some_and(|rest| {
        rest.bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric())
            && rest
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_=.,:/".contains(&b))
    })
}

fn is_mention(word: &str) -> bool {
    word.strip_prefix('@').is_some_and(|rest| {
        rest.bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
            && rest
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
            && !rest.ends_with('.')
    })
}

fn is_issue(word: &str) -> bool {
    word.strip_prefix('#')
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
}

fn is_variable(word: &str) -> bool {
    if let Some(inner) = word
        .strip_prefix("${")
        .and_then(|rest| rest.strip_suffix('}'))
    {
        return is_identifier(inner.split([':', '-', '=']).next().unwrap_or_default());
    }
    if let Some(name) = word.strip_prefix('$') {
        return is_identifier(name);
    }
    word.strip_prefix('%')
        .and_then(|rest| rest.strip_suffix('%'))
        .is_some_and(is_identifier)
}

fn is_identifier(word: &str) -> bool {
    let mut bytes = word.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn is_path(word: &str) -> bool {
    let path_byte = |b: u8| b.is_ascii_alphanumeric() || b"._-/+@~%=,".contains(&b);
    for prefix in ["./", "../", "~/"] {
        if let Some(rest) = word.strip_prefix(prefix) {
            return !rest.is_empty() && rest.bytes().all(path_byte);
        }
    }
    if let Some(rest) = word.strip_prefix('/') {
        return rest
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_')
            && rest.bytes().all(path_byte);
    }
    let bytes = word.as_bytes();
    if bytes.len() > 3
        && bytes[0].is_ascii_alphabetic()
        && &bytes[1..3] == b":\\"
        && bytes[3..].iter().all(|b| path_byte(*b) || *b == b'\\')
    {
        return true;
    }
    word.contains('/')
        && word.bytes().all(path_byte)
        && word.split('/').all(|segment| !segment.is_empty())
        && word.rsplit('/').next().is_some_and(has_extension)
}

fn has_extension(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(stem, extension)| {
        !stem.is_empty()
            && (1..=8).contains(&extension.len())
            && extension.bytes().all(|b| b.is_ascii_alphanumeric())
            && extension.bytes().any(|b| b.is_ascii_alphabetic())
    })
}

fn is_version(word: &str) -> bool {
    let (prefixed, rest) = match word.strip_prefix(['v', 'V']) {
        Some(rest) => (true, rest),
        None => (false, word),
    };
    let numbers = rest.split(['-', '+']).next().unwrap_or_default();
    let suffix = &rest[numbers.len()..];
    let parts: Vec<&str> = numbers.split('.').collect();
    let dots = parts.len() - 1;
    parts
        .iter()
        .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        && (dots >= 2 || (prefixed && dots >= 1))
        && suffix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-+.".contains(&b))
        && !suffix.ends_with(['.', '-', '+'])
}

fn is_hash(word: &str) -> bool {
    (7..=40).contains(&word.len())
        && word.bytes().any(|b| b.is_ascii_digit())
        && (word
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || word
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b)))
        && word.bytes().any(|b| b.is_ascii_alphabetic())
}

fn is_expression(word: &str) -> bool {
    let Some(expression) = Expression::parse(word) else {
        return false;
    };
    if expression.suffixed || expression.strong_separator {
        return true;
    }
    if expression.segments.len() > 1 {
        return !ABBREVIATIONS.contains(&word.to_ascii_lowercase().as_str());
    }
    let name = expression.segments[0];
    is_snake(name) || is_camel(name)
}

fn is_snake(name: &str) -> bool {
    name.contains('_') && name.bytes().any(|b| b.is_ascii_alphanumeric())
}

fn is_camel(name: &str) -> bool {
    name.len() >= 4
        && name.bytes().any(|b| b.is_ascii_lowercase())
        && name.bytes().skip(1).any(|b| b.is_ascii_uppercase())
}

struct Expression<'a> {
    segments: Vec<&'a str>,
    strong_separator: bool,
    suffixed: bool,
}

impl<'a> Expression<'a> {
    fn parse(word: &'a str) -> Option<Expression<'a>> {
        let bytes = word.as_bytes();
        let mut at = 0;
        let mut segments = Vec::new();
        let mut strong_separator = false;
        loop {
            let start = at;
            if !bytes
                .get(at)
                .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
            {
                return None;
            }
            while bytes
                .get(at)
                .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
            {
                at += 1;
            }
            segments.push(&word[start..at]);
            let rest = &word[at..];
            if rest.starts_with("::") || rest.starts_with("->") {
                strong_separator = true;
                at += 2;
            } else if rest.starts_with('.') {
                at += 1;
            } else {
                break;
            }
        }
        let mut suffixed = false;
        while at < bytes.len() {
            let close = match bytes[at] {
                b'(' => b')',
                b'<' => b'>',
                b'[' => b']',
                _ => return None,
            };
            let end = matching(bytes, at, close)?;
            let inner = &word[at + 1..end];
            if bytes[at] == b'(' && (inner == "s" || inner == "es") && end + 1 == bytes.len() {
                return None;
            }
            suffixed = true;
            at = end + 1;
        }
        Some(Expression {
            segments,
            strong_separator,
            suffixed,
        })
    }
}

fn matching(bytes: &[u8], open_at: usize, close: u8) -> Option<usize> {
    let open = bytes[open_at];
    let mut depth = 0usize;
    for (index, byte) in bytes.iter().enumerate().skip(open_at) {
        if *byte == open {
            depth += 1;
        } else if *byte == close {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}
