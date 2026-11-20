struct Parts<'a> {
    authority: &'a str,
    rest: &'a str,
}

fn split(url: &str) -> Option<Parts<'_>> {
    let (scheme, after) = url.split_once("://")?;
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return None;
    }
    let end = after.find(['/', '?', '#']).unwrap_or(after.len());
    let (authority, rest) = after.split_at(end);
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    Some(Parts { authority, rest })
}

fn strip_port(authority: &str) -> &str {
    if authority.starts_with('[') {
        return authority
            .find(']')
            .map_or(authority, |end| &authority[..=end]);
    }
    authority
        .split_once(':')
        .map_or(authority, |(host, _)| host)
}

pub(crate) fn host(url: &str) -> Option<String> {
    let parts = split(url)?;
    let host = strip_port(parts.authority).trim_end_matches('.');
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

pub(crate) fn key(url: &str) -> String {
    match split(url) {
        Some(parts) => {
            let authority = parts.authority.to_ascii_lowercase();
            let authority = authority.strip_prefix("www.").unwrap_or(&authority);
            format!("{authority}{}", parts.rest)
        }
        None => url.to_owned(),
    }
}

pub(crate) fn input_key(input: &str) -> String {
    let input = input.trim();
    if split(input).is_some() {
        return key(input);
    }
    let end = input.find(['/', '?', '#']).unwrap_or(input.len());
    let (authority, rest) = input.split_at(end);
    let authority = authority.to_ascii_lowercase();
    let authority = authority.strip_prefix("www.").unwrap_or(&authority);
    format!("{authority}{rest}")
}

pub(crate) fn prefix_end(prefix: &str) -> String {
    format!("{prefix}\u{10FFFF}")
}
