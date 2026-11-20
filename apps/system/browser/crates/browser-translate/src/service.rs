use rikka_languages::Language;

use crate::limits::Limits;
use crate::markup::Dialect;

#[derive(Debug)]
pub struct Service {
    pub id: &'static str,
    pub label: &'static str,
    pub dialect: Dialect,
    pub targets: &'static [&'static str],
    pub aliases: &'static [(&'static str, &'static str)],
    pub limits: Limits,
}

impl Service {
    #[must_use]
    pub fn serves(&self, language: &Language) -> bool {
        self.targets.contains(&language.tag())
    }

    #[must_use]
    pub fn code(&self, language: &Language) -> &'static str {
        self.aliases
            .iter()
            .find(|(tag, _)| *tag == language.tag())
            .map_or(language.tag(), |(_, code)| code)
    }

    #[must_use]
    pub fn language(&self, code: &str) -> Option<Language> {
        self.aliases
            .iter()
            .find(|(_, alias)| alias.eq_ignore_ascii_case(code))
            .and_then(|(tag, _)| Language::from_tag(tag))
            .or_else(|| Language::lookup(code))
    }
}
