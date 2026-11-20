use std::fmt;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    Address,
    CatchAll,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Identifier(String);

impl Identifier {
    pub fn address(email: &str) -> Result<Identifier> {
        let (local, domain) = split_email(email)?;
        let base = local.split_once('+').map_or(local, |(base, _)| base);
        let canonical: String = base
            .chars()
            .filter(|character| !matches!(character, '.' | '_' | '-'))
            .collect();
        Ok(Identifier(format!(
            "{}@{}",
            canonical.to_lowercase(),
            domain.to_lowercase()
        )))
    }

    pub fn catch_all(email: &str) -> Result<Identifier> {
        Ok(Identifier(format!("@{}", email_domain(email)?)))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn is_catch_all(&self) -> bool {
        self.0.starts_with('@')
    }

    pub(crate) fn for_scope(email: &str, scope: Scope) -> Result<Identifier> {
        match scope {
            Scope::Address => Identifier::address(email),
            Scope::CatchAll => Identifier::catch_all(email),
        }
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub(crate) fn email_domain(email: &str) -> Result<String> {
    split_email(email).map(|(_, domain)| domain.to_lowercase())
}

fn split_email(email: &str) -> Result<(&str, &str)> {
    email
        .rsplit_once('@')
        .filter(|(_, domain)| !domain.is_empty())
        .ok_or_else(|| Error::InvalidEmail(email.to_owned()))
}
