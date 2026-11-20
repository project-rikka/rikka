use std::fmt;

const EVERY_CATEGORY: &str = "LC_ALL";
const FALLBACK: &str = "LANG";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LocaleCategory {
    Messages,
    Ctype,
    Time,
    Measurement,
}

impl LocaleCategory {
    #[must_use]
    pub const fn variable(self) -> &'static str {
        match self {
            LocaleCategory::Messages => "LC_MESSAGES",
            LocaleCategory::Ctype => "LC_CTYPE",
            LocaleCategory::Time => "LC_TIME",
            LocaleCategory::Measurement => "LC_MEASUREMENT",
        }
    }

    #[must_use]
    pub fn posix_name(self) -> Option<String> {
        self.posix_name_with(|variable| std::env::var(variable).ok())
    }

    #[must_use]
    pub fn posix_name_with(self, lookup: impl Fn(&str) -> Option<String>) -> Option<String> {
        [EVERY_CATEGORY, self.variable(), FALLBACK]
            .into_iter()
            .filter_map(lookup)
            .find(|value| !value.is_empty())
    }

    #[must_use]
    pub fn locale(self) -> Option<PosixLocale> {
        self.posix_name().and_then(PosixLocale::parse)
    }

    #[must_use]
    pub fn locale_with(self, lookup: impl Fn(&str) -> Option<String>) -> Option<PosixLocale> {
        self.posix_name_with(lookup).and_then(PosixLocale::parse)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PosixLocale {
    name: String,
    language: usize,
    territory: Option<(usize, usize)>,
}

impl PosixLocale {
    #[must_use]
    pub fn parse(name: impl Into<String>) -> Option<PosixLocale> {
        let name = name.into();
        let plain = name.split(['.', '@']).next().unwrap_or_default();
        let (language, territory) = match plain.split_once(['_', '-']) {
            Some((language, territory)) => (language, Some(territory)),
            None => (plain, None),
        };
        let spoken = matches!(language.len(), 2..=3)
            && language.bytes().all(|byte| byte.is_ascii_alphabetic());
        if !spoken {
            return None;
        }
        let language = language.len();
        let territory = territory
            .map(|territory| territory.split(['_', '-']).next_back().unwrap_or(territory))
            .filter(|territory| {
                territory.len() == 2 && territory.bytes().all(|byte| byte.is_ascii_alphabetic())
            })
            .map(|territory| (plain.len() - territory.len(), plain.len()));
        Some(PosixLocale {
            name,
            language,
            territory,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn language(&self) -> &str {
        &self.name[..self.language]
    }

    #[must_use]
    pub fn territory(&self) -> Option<&str> {
        self.territory.map(|(start, end)| &self.name[start..end])
    }

    #[must_use]
    pub fn tag(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for PosixLocale {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.language().to_ascii_lowercase())?;
        match self.territory() {
            Some(territory) => write!(formatter, "-{}", territory.to_ascii_uppercase()),
            None => Ok(()),
        }
    }
}
