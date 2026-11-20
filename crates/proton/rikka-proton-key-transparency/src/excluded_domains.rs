const EXCLUDED_DOMAINS: &str = include_str!("../data/no_kt_domains.txt");

pub(crate) fn is_excluded(domain: &str) -> bool {
    EXCLUDED_DOMAINS.lines().any(|excluded| excluded == domain)
}
