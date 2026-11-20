use crate::suffix::suffix;

#[must_use]
pub fn registrable_domain(domain: &str) -> Option<&str> {
    let suffix = suffix(domain)?.as_str();
    let rest = domain.strip_suffix(suffix)?.strip_suffix('.')?;
    let label = rest.rfind('.').map_or(0, |dot| dot + 1);
    Some(&domain[label..])
}

#[must_use]
pub fn site(domain: &str) -> &str {
    registrable_domain(domain).unwrap_or(domain)
}

#[must_use]
pub fn is_public_suffix(domain: &str) -> bool {
    suffix(domain).is_some_and(|suffix| suffix.is_listed() && suffix.as_str().len() == domain.len())
}
