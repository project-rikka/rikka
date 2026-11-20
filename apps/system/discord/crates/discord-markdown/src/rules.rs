pub(crate) struct List {
    pub(crate) start: Option<u32>,
    pub(crate) items: Vec<String>,
}

pub(crate) struct CustomEmoji<'a> {
    pub(crate) animated: bool,
    pub(crate) name: &'a str,
    pub(crate) id: &'a str,
}

pub(crate) fn heading(src: &str) -> Option<(usize, u8, &str)> {
    let bytes = src.as_bytes();
    let mut at = src.len() - src.trim_start_matches(' ').len();
    let hashes = src[at..].len() - src[at..].trim_start_matches('#').len();
    if !(1..=3).contains(&hashes) {
        return None;
    }
    at += hashes;
    let gap = src[at..].len() - src[at..].trim_start_matches([' ', '\t']).len();
    if gap == 0 {
        return None;
    }
    at += gap;
    if matches!(bytes.get(at), None | Some(b'#' | b'\n')) {
        return None;
    }
    let content_start = at;
    let line_end = src[content_start..]
        .find('\n')
        .map_or(src.len(), |offset| content_start + offset);
    let ends = src[content_start..line_end]
        .char_indices()
        .skip(1)
        .map(|(offset, _)| content_start + offset)
        .chain(std::iter::once(line_end));
    for end in ends {
        if let Some(consumed) = heading_tail(&src[end..]) {
            return Some((
                end + consumed,
                hashes as u8,
                src[content_start..end].trim_end(),
            ));
        }
    }
    None
}

fn heading_tail(rest: &str) -> Option<usize> {
    let hashes = rest.len() - rest.trim_start_matches('#').len();
    let after = &rest[hashes..];
    let space = after.len() - after.trim_start().len();
    if space == after.len() {
        return Some(rest.len());
    }
    after[..space]
        .rfind('\n')
        .map(|newline| hashes + newline + 1)
}

pub(crate) fn subtext(src: &str) -> Option<(usize, &str)> {
    let rest = src.strip_prefix("-#")?;
    let content = rest.trim_start_matches(' ');
    if content.len() == rest.len() {
        return None;
    }
    let skipped = src.len() - content.len();
    let line = content.split('\n').next().unwrap_or_default();
    if line.is_empty() {
        return None;
    }
    let consumed = skipped + line.len() + usize::from(content.len() > line.len());
    Some((consumed, line))
}

pub(crate) fn quote(src: &str) -> Option<(usize, String)> {
    let indent = src.len() - src.trim_start_matches(' ').len();
    let start = &src[indent..];
    if let Some(rest) = start.strip_prefix(">>> ") {
        return Some((src.len(), rest.to_owned()));
    }
    let mut consumed = 0;
    let mut lines = Vec::new();
    for line in src.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let Some(content) = quoted_line(body) else {
            break;
        };
        lines.push(content);
        consumed += line.len();
    }
    if lines.is_empty() {
        return None;
    }
    Some((consumed, lines.join("\n")))
}

fn quoted_line(line: &str) -> Option<&str> {
    let trimmed = line.trim_start_matches(' ');
    let rest = trimmed.strip_prefix('>')?;
    if rest.starts_with(">>") {
        return None;
    }
    rest.strip_prefix(' ')
}

pub(crate) fn list(src: &str) -> Option<(usize, List)> {
    let indent = src.len() - src.trim_start_matches(' ').len();
    let (bullet, ordered) = bullet(&src[indent..])?;
    let content = src[indent + bullet..].trim_start_matches(' ');
    if content.is_empty() || content.starts_with('\n') {
        return None;
    }
    let mut end = src.len();
    let mut consumed = src.len();
    let mut search = indent + bullet + 1;
    while let Some(offset) = src[search..].find('\n') {
        let newline = search + offset;
        let next = &src[newline + 1..];
        let continues = next.starts_with(' ') || (indent == 0 && bullet_with_space(next).is_some());
        if !continues {
            end = newline;
            consumed = newline + 1;
            break;
        }
        search = newline + 1;
    }
    let mut items: Vec<String> = Vec::new();
    for line in src[..end].split('\n') {
        let line_indent = line.len() - line.trim_start_matches(' ').len();
        if line_indent == indent
            && let Some(width) = bullet_with_space(&line[indent..])
        {
            items.push(line[indent + width..].to_owned());
            continue;
        }
        let strip = line_indent.min(indent + bullet + 1);
        if let Some(item) = items.last_mut() {
            item.push('\n');
            item.push_str(&line[strip..]);
        }
    }
    let start = ordered.then(|| src[indent..indent + bullet - 1].parse::<u32>().unwrap_or(1));
    Some((consumed, List { start, items }))
}

fn bullet(src: &str) -> Option<(usize, bool)> {
    let bytes = src.as_bytes();
    let (width, ordered) = match bytes.first()? {
        b'-' | b'*' => (1, false),
        b'0'..=b'9' => {
            let digits = bytes
                .iter()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
            if digits > 9 || bytes.get(digits) != Some(&b'.') {
                return None;
            }
            (digits + 1, true)
        }
        _ => return None,
    };
    (bytes.get(width) == Some(&b' ')).then_some((width, ordered))
}

fn bullet_with_space(src: &str) -> Option<usize> {
    bullet(src).map(|(width, _)| width + 1)
}

pub(crate) fn code_block(src: &str) -> Option<(usize, Option<&str>, &str)> {
    let rest = src.strip_prefix("```")?;
    let language = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-' | '.' | '#')))
        .filter(|&length| length > 0 && rest[length..].starts_with('\n'));
    if let Some(length) = language
        && let Some((end, text)) = code_body(src, 3 + length + 1)
    {
        return Some((end, Some(&rest[..length]), text));
    }
    code_body(src, 3).map(|(end, text)| (end, None, text))
}

fn code_body(src: &str, from: usize) -> Option<(usize, &str)> {
    let start = from + src[from..].len() - src[from..].trim_start_matches('\n').len();
    let mut end = start + src[start..].chars().next()?.len_utf8();
    loop {
        let after = src[end..].trim_start_matches('\n');
        if after.starts_with("```") {
            return Some((src.len() - after.len() + 3, &src[start..end]));
        }
        end += src[end..].chars().next()?.len_utf8();
    }
}

pub(crate) fn escape(src: &str) -> Option<(usize, &str)> {
    let rest = src.strip_prefix('\\')?;
    let escaped = rest.chars().next()?;
    if escaped.is_ascii_alphanumeric() || escaped.is_whitespace() {
        return None;
    }
    Some((1 + escaped.len_utf8(), &rest[..escaped.len_utf8()]))
}

pub(crate) fn autolink(src: &str) -> Option<(usize, &str)> {
    let rest = src.strip_prefix('<')?;
    let close = rest.find('>')?;
    let target = &rest[..close];
    let colon = target.find(':')?;
    if colon == 0
        || target[..colon].contains([' ', '>'])
        || !target[colon + 1..].starts_with('/')
        || target.contains(' ')
        || target.len() < colon + 3
    {
        return None;
    }
    Some((close + 2, target))
}

pub(crate) fn url(src: &str) -> Option<(usize, &str)> {
    let scheme = if src.starts_with("https://") {
        8
    } else if src.starts_with("http://") {
        7
    } else {
        return None;
    };
    let run = src[scheme..]
        .find(|c: char| c.is_whitespace() || c == '<')
        .map_or(src.len(), |length| scheme + length);
    let (last, c) = src[..run]
        .char_indices()
        .rev()
        .find(|&(_, c)| !matches!(c, '.' | ',' | ':' | ';' | '"' | '\'' | ']'))?;
    if last <= scheme {
        return None;
    }
    let mut url = &src[..last + c.len_utf8()];
    while url.ends_with(')') && url.matches('(').count() < url.matches(')').count() {
        url = &url[..url.len() - 1];
    }
    (url.len() > scheme + 1).then_some((url.len(), url))
}

pub(crate) fn link(src: &str) -> Option<(usize, &str, &str)> {
    let rest = src.strip_prefix('[')?;
    link_label_ends(rest)
        .into_iter()
        .rev()
        .find_map(|label_end| link_target(src, label_end))
}

fn link_target(src: &str, label_end: usize) -> Option<(usize, &str, &str)> {
    let label = &src[1..1 + label_end];
    let after = src[1 + label_end..].strip_prefix("](")?;
    let mut at = src.len() - after.trim_start().len();
    if src[at..].starts_with('<') {
        at += 1;
    }
    let href_start = at;
    loop {
        if let Some(close) = link_tail(&src[at..]) {
            return Some((at + close, label, &src[href_start..at]));
        }
        let piece = &src[at..];
        let c = piece.chars().next()?;
        if c == '('
            && let Some(group) = piece[1..].find(')')
        {
            at += group + 2;
            continue;
        }
        if c == '\\' {
            at += 1 + piece[1..].chars().next()?.len_utf8();
            continue;
        }
        if c.is_whitespace() {
            return None;
        }
        at += c.len_utf8();
    }
}

fn link_label_ends(rest: &str) -> Vec<usize> {
    let mut ends = Vec::new();
    let mut at = 0;
    while let Some(c) = rest[at..].chars().next() {
        match c {
            '[' => match rest[at + 1..].find(']') {
                Some(close) => at += close + 2,
                None => break,
            },
            ']' => {
                ends.push(at);
                let ahead = &rest[at + 1..];
                match ahead.find(['[', ']']) {
                    Some(end) if ahead[end..].starts_with(']') => at += 1,
                    _ => break,
                }
            }
            _ => at += c.len_utf8(),
        }
    }
    ends
}

fn link_tail(rest: &str) -> Option<usize> {
    let mut at = usize::from(rest.starts_with('>'));
    let spaced = rest[at..].len() - rest[at..].trim_start().len();
    if spaced > 0
        && let Some(quote) = rest[at + spaced..].chars().next()
        && matches!(quote, '"' | '\'')
    {
        let title_start = at + spaced + 1;
        let mut search = title_start;
        while let Some(offset) = rest[search..].find(['"', '\'']) {
            let after = &rest[search + offset + 1..];
            let gap = after.len() - after.trim_start().len();
            if after[gap..].starts_with(')') {
                return Some(search + offset + 1 + gap + 1);
            }
            search += offset + 1;
        }
    }
    at += spaced;
    rest[at..].starts_with(')').then_some(at + 1)
}

pub(crate) fn italic(src: &str) -> Option<(usize, &str)> {
    if src.starts_with('*') {
        italic_star(src)
    } else if src.starts_with('_') {
        italic_underscore(src)
    } else {
        None
    }
}

fn italic_star(src: &str) -> Option<(usize, &str)> {
    let bytes = src.as_bytes();
    if src[1..].chars().next()?.is_whitespace() {
        return None;
    }
    let mut at = 1;
    loop {
        if at > 1 && bytes.get(at) == Some(&b'*') && bytes.get(at + 1) != Some(&b'*') {
            return Some((at + 1, &src[1..at]));
        }
        let rest = &src[at..];
        if rest.starts_with("**") {
            at += 2;
            continue;
        }
        let c = rest.chars().next()?;
        if c == '\\' {
            at += 1 + rest[1..].chars().next()?.len_utf8();
            continue;
        }
        if c.is_whitespace() {
            let spaces = rest.len() - rest.trim_start().len();
            let after = &rest[spaces..];
            if after.starts_with("**") {
                at += spaces + 2;
                continue;
            }
            let next = after.chars().next()?;
            if next == '*' {
                return None;
            }
            if next == '\\' {
                at += spaces + 1 + after[1..].chars().next()?.len_utf8();
                continue;
            }
            at += spaces + next.len_utf8();
            continue;
        }
        if c == '*' {
            return None;
        }
        at += c.len_utf8();
    }
}

fn italic_underscore(src: &str) -> Option<(usize, &str)> {
    let bytes = src.as_bytes();
    let mut at = 1;
    loop {
        if at > 1
            && bytes.get(at) == Some(&b'_')
            && !bytes.get(at + 1).copied().is_some_and(is_word)
        {
            return Some((at + 1, &src[1..at]));
        }
        let rest = &src[at..];
        if rest.starts_with("__") {
            at += 2;
            continue;
        }
        let c = rest.chars().next()?;
        match c {
            '\\' => at += 1 + rest[1..].chars().next()?.len_utf8(),
            '_' => return None,
            _ => at += c.len_utf8(),
        }
    }
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

pub(crate) fn bold(src: &str) -> Option<(usize, &str)> {
    doubled(src, "**", Some(b'*'))
}

pub(crate) fn underline(src: &str) -> Option<(usize, &str)> {
    doubled(src, "__", Some(b'_'))
}

fn doubled<'a>(src: &'a str, mark: &str, not_after: Option<u8>) -> Option<(usize, &'a str)> {
    let rest = src.strip_prefix(mark)?;
    let bytes = src.as_bytes();
    let mut at = mark.len();
    loop {
        let piece = &src[at..];
        if at > mark.len()
            && piece.starts_with(mark)
            && not_after.is_none_or(|refused| bytes.get(at + mark.len()) != Some(&refused))
        {
            return Some((at + mark.len(), &rest[..at - mark.len()]));
        }
        let c = piece.chars().next()?;
        if c == '\\' {
            at += 1 + piece[1..].chars().next()?.len_utf8();
        } else {
            at += c.len_utf8();
        }
    }
}

pub(crate) fn strike(src: &str) -> Option<(usize, &str)> {
    lazy_pair(src, "~~", Some(b'_'))
}

pub(crate) fn spoiler(src: &str) -> Option<(usize, &str)> {
    lazy_pair(src, "||", None)
}

fn lazy_pair<'a>(src: &'a str, mark: &str, not_after: Option<u8>) -> Option<(usize, &'a str)> {
    let rest = src.strip_prefix(mark)?;
    let bytes = src.as_bytes();
    for (offset, _) in rest.char_indices().skip(1) {
        let at = mark.len() + offset;
        if src[at..].starts_with(mark)
            && not_after.is_none_or(|refused| bytes.get(at + mark.len()) != Some(&refused))
        {
            return Some((at + mark.len(), &rest[..offset]));
        }
    }
    None
}

pub(crate) fn inline_code(src: &str) -> Option<(usize, String)> {
    let run = src.len() - src.trim_start_matches('`').len();
    if run == 0 {
        return None;
    }
    for ticks in (1..=run).rev() {
        let fence = &src[..ticks];
        let body = &src[ticks..];
        for (offset, c) in body.char_indices() {
            let end = ticks + offset + c.len_utf8();
            if c == '`' {
                continue;
            }
            let after = &src[end..];
            if after.starts_with(fence) && !after[ticks..].starts_with('`') {
                return Some((end + ticks, trim_code(&src[ticks..end])));
            }
        }
    }
    None
}

fn trim_code(code: &str) -> String {
    let mut code = code;
    if code.starts_with(' ') && code.trim_start_matches(' ').starts_with('`') {
        code = &code[1..];
    }
    if code.ends_with(' ') && code.trim_end_matches(' ').ends_with('`') {
        code = &code[..code.len() - 1];
    }
    code.to_owned()
}

pub(crate) fn user(src: &str) -> Option<(usize, &str)> {
    let rest = src.strip_prefix("<@")?;
    let rest = rest.strip_prefix('!').unwrap_or(rest);
    let skipped = src.len() - rest.len();
    snowflake_then_close(rest).map(|(length, id)| (skipped + length, id))
}

pub(crate) fn role(src: &str) -> Option<(usize, &str)> {
    let rest = src.strip_prefix("<@&")?;
    snowflake_then_close(rest).map(|(length, id)| (3 + length, id))
}

pub(crate) fn channel(src: &str) -> Option<(usize, &str)> {
    let rest = src.strip_prefix("<#")?;
    snowflake_then_close(rest).map(|(length, id)| (2 + length, id))
}

fn snowflake_then_close(rest: &str) -> Option<(usize, &str)> {
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    (digits > 0 && rest[digits..].starts_with('>')).then(|| (digits + 1, &rest[..digits]))
}

pub(crate) fn custom_emoji(src: &str) -> Option<(usize, CustomEmoji<'_>)> {
    let rest = src.strip_prefix('<')?;
    let (animated, rest) = match rest.strip_prefix('a') {
        Some(after) => (true, after),
        None => (false, rest),
    };
    let rest = rest.strip_prefix(':')?;
    let name_length = rest.len()
        - rest
            .trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_')
            .len();
    if !(1..=32).contains(&name_length) {
        return None;
    }
    let name = &rest[..name_length];
    let after = rest[name_length..].strip_prefix(':')?;
    let (length, id) = snowflake_then_close(after)?;
    let consumed = src.len() - after.len() + length;
    Some((consumed, CustomEmoji { animated, name, id }))
}

pub(crate) fn timestamp(src: &str) -> Option<(usize, i64, Option<char>)> {
    let rest = src.strip_prefix("<t:")?;
    let negative = rest.starts_with('-');
    let digits_from = usize::from(negative);
    let digits = rest[digits_from..].len()
        - rest[digits_from..]
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .len();
    if !(1..=17).contains(&digits) {
        return None;
    }
    let number_end = digits_from + digits;
    let unix: i64 = rest[..number_end].parse().ok()?;
    let after = &rest[number_end..];
    if after.starts_with('>') {
        return Some((3 + number_end + 1, unix, None));
    }
    let style = after.strip_prefix(':')?.chars().next()?;
    if !matches!(style, 't' | 'T' | 'd' | 'D' | 'f' | 'F' | 'R') || !after[2..].starts_with('>') {
        return None;
    }
    Some((3 + number_end + 3, unix, Some(style)))
}

pub(crate) fn command(src: &str) -> Option<(usize, &str)> {
    let rest = src.strip_prefix("</")?;
    let colon = rest.find(':')?;
    let name = &rest[..colon];
    if name.is_empty() || name.contains(['>', '\n']) {
        return None;
    }
    let (length, _) = snowflake_then_close(&rest[colon + 1..])?;
    Some((2 + colon + 1 + length, name))
}

pub(crate) fn text(src: &str) -> usize {
    let mut word_end = 0;
    let mut word_names_scheme = false;
    for (at, c) in src.char_indices().skip(1) {
        if c == '\n' || stops_text(c) || (src[at..].starts_with("  ") && breaks_line(&src[at..])) {
            return at;
        }
        if !c.is_ascii_alphanumeric() {
            continue;
        }
        if at >= word_end {
            let run = src[at..]
                .find(|w: char| !(w.is_ascii_alphanumeric() || w == '_'))
                .map_or(src.len(), |length| at + length);
            word_end = run;
            word_names_scheme = src[run..]
                .strip_prefix(':')
                .and_then(|tail| tail.chars().next())
                .is_some_and(|next| !next.is_whitespace());
        }
        if word_names_scheme {
            return at;
        }
    }
    src.len()
}

fn stops_text(c: char) -> bool {
    (c as u32) < 0xC0 && !c.is_ascii_alphanumeric() && !c.is_whitespace()
}

fn breaks_line(src: &str) -> bool {
    src.trim_start_matches(' ').starts_with('\n')
}
