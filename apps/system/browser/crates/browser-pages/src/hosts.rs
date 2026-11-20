use url::Url;

#[must_use]
pub fn host_key(host: &str) -> String {
    host.trim().trim_end_matches('.').to_ascii_lowercase()
}

#[must_use]
pub fn web_host(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = host_key(parsed.host_str()?);
    (!host.is_empty()).then_some(host)
}

#[must_use]
pub fn covers(domain: &str, host: &str) -> bool {
    !domain.is_empty()
        && (host == domain
            || host
                .strip_suffix(domain)
                .is_some_and(|rest| rest.ends_with('.')))
}

#[must_use]
pub fn registrable_site(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed.host()?;
    let url::Host::Domain(domain) = host else {
        return Some(host.to_string());
    };
    let domain = host_key(domain);
    Some(rikka_public_suffix::site(&domain).to_owned())
}

#[must_use]
pub fn site_of(host: &str) -> String {
    let host = host_key(host);
    match host.strip_prefix("www.") {
        Some(site) => site.to_owned(),
        None => host,
    }
}

#[must_use]
pub fn web_site(url: &str) -> Option<String> {
    web_host(url).map(|host| site_of(&host))
}
