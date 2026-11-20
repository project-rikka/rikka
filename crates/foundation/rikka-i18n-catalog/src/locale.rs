use std::fmt;
use std::str::FromStr;

use crate::error::Error;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Locale {
    language: Box<str>,
    script: Option<Box<str>>,
    region: Option<Box<str>>,
}

impl Locale {
    pub fn und() -> Self {
        Self {
            language: "und".into(),
            script: None,
            region: None,
        }
    }

    pub fn system() -> Option<Self> {
        crate::system::tag()?.parse().ok()
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    pub fn script(&self) -> Option<&str> {
        self.script.as_deref()
    }

    pub fn region(&self) -> Option<&str> {
        self.region.as_deref()
    }

    pub fn parent(&self) -> Option<Self> {
        if self.region.is_some() {
            return Some(Self {
                region: None,
                ..self.clone()
            });
        }
        self.script.as_ref().map(|_| Self {
            script: None,
            ..self.clone()
        })
    }

    pub fn ancestry(&self) -> impl Iterator<Item = Self> {
        std::iter::successors(Some(self.clone()), Self::parent)
    }
}

impl Default for Locale {
    fn default() -> Self {
        Self::und()
    }
}

impl FromStr for Locale {
    type Err = Error;

    fn from_str(tag: &str) -> Result<Self, Self::Err> {
        let invalid = || Error::InvalidLocale(tag.to_owned());
        let mut subtags = tag
            .split(['-', '_'])
            .map(|subtag| subtag.split(['.', '@']).next().unwrap_or(subtag))
            .filter(|subtag| !subtag.is_empty());

        let language = subtags.next().ok_or_else(invalid)?;
        if !matches!(language.len(), 2..=3) || !language.bytes().all(|b| b.is_ascii_alphabetic()) {
            return Err(invalid());
        }

        let mut locale = Self {
            language: language.to_ascii_lowercase().into(),
            script: None,
            region: None,
        };
        for subtag in subtags {
            let bytes = subtag.as_bytes();
            match subtag.len() {
                4 if bytes.iter().all(u8::is_ascii_alphabetic) && locale.script.is_none() => {
                    let mut script = subtag.to_ascii_lowercase();
                    script[..1].make_ascii_uppercase();
                    locale.script = Some(script.into());
                }
                2 if bytes.iter().all(u8::is_ascii_alphabetic) && locale.region.is_none() => {
                    locale.region = Some(subtag.to_ascii_uppercase().into());
                }
                3 if bytes.iter().all(u8::is_ascii_digit) && locale.region.is_none() => {
                    locale.region = Some(subtag.into());
                }
                _ => break,
            }
        }
        Ok(locale)
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.language)?;
        if let Some(script) = &self.script {
            write!(f, "-{script}")?;
        }
        if let Some(region) = &self.region {
            write!(f, "-{region}")?;
        }
        Ok(())
    }
}

pub trait IntoLocale {
    fn into_locale(self) -> Result<Locale, Error>;
}

impl IntoLocale for Locale {
    fn into_locale(self) -> Result<Locale, Error> {
        Ok(self)
    }
}

impl IntoLocale for &Locale {
    fn into_locale(self) -> Result<Locale, Error> {
        Ok(self.clone())
    }
}

impl IntoLocale for &str {
    fn into_locale(self) -> Result<Locale, Error> {
        self.parse()
    }
}

impl IntoLocale for &String {
    fn into_locale(self) -> Result<Locale, Error> {
        self.parse()
    }
}

impl IntoLocale for String {
    fn into_locale(self) -> Result<Locale, Error> {
        self.parse()
    }
}

pub fn negotiate<'a>(
    requested: impl IntoIterator<Item = &'a Locale>,
    available: &[Locale],
) -> Option<Locale> {
    for preference in requested {
        if let Some(exact) = preference
            .ancestry()
            .find(|candidate| available.contains(candidate))
        {
            return Some(exact);
        }
        let related = available.iter().find(|candidate| {
            candidate.language == preference.language
                && (preference.script.is_none() || candidate.script == preference.script)
        });
        if let Some(related) = related {
            return Some(related.clone());
        }
    }
    None
}
