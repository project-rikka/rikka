use url::Host;

const LOCAL_SUFFIXES: [&str; 8] = [
    "localhost",
    "local",
    "localdomain",
    "test",
    "internal",
    "lan",
    "home.arpa",
    "onion",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostClass {
    Address,
    Local,
    Public,
    NonUnique,
}

impl HostClass {
    pub(crate) fn exempt_from_upgrade(self) -> bool {
        self != HostClass::Public
    }
}

pub(crate) fn class(host: &Host<&str>) -> HostClass {
    match host {
        Host::Ipv4(_) | Host::Ipv6(_) => HostClass::Address,
        Host::Domain(domain) => domain_class(domain),
    }
}

pub(crate) fn domain_class(domain: &str) -> HostClass {
    let domain = domain.trim_end_matches('.');
    if is_local(domain) {
        return HostClass::Local;
    }
    if has_public_registrable(domain) {
        HostClass::Public
    } else {
        HostClass::NonUnique
    }
}

fn is_local(domain: &str) -> bool {
    LOCAL_SUFFIXES.iter().any(|suffix| {
        (domain == *suffix && *suffix == "localhost")
            || domain
                .strip_suffix(suffix)
                .is_some_and(|front| front.len() > 1 && front.ends_with('.'))
    })
}

fn has_public_registrable(domain: &str) -> bool {
    rikka_public_suffix::icann_suffix(domain)
        .is_some_and(|suffix| domain.len() > suffix.as_str().len() + 1)
}

pub(crate) fn is_public_suffix(domain: &str) -> bool {
    rikka_public_suffix::is_public_suffix(domain.trim_end_matches('.'))
}

pub(crate) fn registrable(domain: &str) -> Option<&str> {
    let domain = domain.trim_end_matches('.');
    rikka_public_suffix::suffix(domain)
        .is_some_and(|suffix| suffix.is_listed())
        .then(|| rikka_public_suffix::registrable_domain(domain))
        .flatten()
}

pub(crate) fn is_strict_label_host(domain: &str) -> bool {
    let domain = domain.trim_end_matches('.');
    !domain.is_empty()
        && domain.len() <= 253
        && domain.split('.').all(|label| {
            (1..=63).contains(&label.len())
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
        && domain
            .rsplit('.')
            .next()
            .is_some_and(|last| last.bytes().any(|byte| byte.is_ascii_lowercase()))
}

pub(crate) fn is_plausible_host(domain: &str) -> bool {
    let domain = domain.trim_end_matches('.');
    !domain.is_empty()
        && domain.len() <= 253
        && domain.split('.').all(|label| {
            (1..=63).contains(&label.len())
                && label.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || byte == b'-'
                        || byte == b'_'
                })
        })
}
