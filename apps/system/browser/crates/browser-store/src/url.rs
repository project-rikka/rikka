fn authority(url: &str) -> Option<&str> {
    let (scheme, after) = url.split_once("://")?;
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return None;
    }
    let end = after.find(['/', '?', '#']).unwrap_or(after.len());
    let authority = &after[..end];
    Some(
        authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host),
    )
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
    let host = strip_port(authority(url)?).trim_end_matches('.');
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}
