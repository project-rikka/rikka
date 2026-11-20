const PROTON_DOMAINS: [&str; 4] = ["protonmail.com", "protonmail.ch", "pm.me", "proton.me"];
const GMAIL_DOMAINS: [&str; 3] = ["gmail.com", "googlemail.com", "google.com"];
const PLUS_DOMAINS: [&str; 6] = [
    "hotmail.com",
    "hotmail.co.uk",
    "hotmail.fr",
    "outlook.com",
    "yandex.ru",
    "mail.ru",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CanonicalScheme {
    Default,
    Plus,
    Gmail,
    Proton,
}

#[must_use]
pub fn canonical_email(email: &str, scheme: CanonicalScheme) -> String {
    let (local, domain) = match email.rsplit_once('@') {
        Some((local, domain)) => (local, Some(domain)),
        None => (email, None),
    };
    let without_alias = || local.split_once('+').map_or(local, |(base, _)| base);
    let normalized = match scheme {
        CanonicalScheme::Default => return email.to_lowercase(),
        CanonicalScheme::Proton => without_alias()
            .chars()
            .filter(|character| !matches!(character, '.' | '_' | '-'))
            .collect::<String>()
            .to_lowercase(),
        CanonicalScheme::Gmail => without_alias().replace('.', "").to_lowercase(),
        CanonicalScheme::Plus => without_alias().to_lowercase(),
    };
    match domain {
        Some(domain) => format!("{normalized}@{}", domain.to_lowercase()),
        None => normalized,
    }
}

#[must_use]
pub fn canonical_internal_email(email: &str) -> String {
    canonical_email(email, CanonicalScheme::Proton)
}

#[must_use]
pub fn canonical_email_by_guess(email: &str) -> String {
    let domain = email
        .rsplit_once('@')
        .map(|(_, domain)| domain.to_lowercase())
        .unwrap_or_default();
    let scheme = if PROTON_DOMAINS.contains(&domain.as_str()) {
        CanonicalScheme::Proton
    } else if GMAIL_DOMAINS.contains(&domain.as_str()) {
        CanonicalScheme::Gmail
    } else if PLUS_DOMAINS.contains(&domain.as_str()) {
        CanonicalScheme::Plus
    } else {
        CanonicalScheme::Default
    };
    canonical_email(email, scheme)
}

#[must_use]
pub fn user_id_email(user_id: &str) -> Option<&str> {
    let (_, after_open) = user_id.split_once('<')?;
    let (email, _) = after_open.split_once('>')?;
    Some(email)
}
