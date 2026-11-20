use rikka_i18n::current_locale;
use rikka_languages::Language as Listed;
use rikka_ui::{Label, tr};

const OFFERED: [&str; 33] = [
    "en", "ja", "zh-Hans", "zh-Hant", "ko", "es", "fr", "de", "it", "pt", "ru", "uk", "pl", "nl",
    "sv", "da", "nb", "fi", "cs", "hu", "ro", "el", "tr", "ar", "he", "fa", "hi", "bn", "th", "vi",
    "id", "ms", "fil",
];

const UNDETERMINED: &str = "und";

const AUTO: &str = "shell.translate.auto";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Language {
    Auto,
    Known(Listed),
    Custom(String),
}

impl Language {
    pub(super) fn offered() -> impl Iterator<Item = Language> {
        OFFERED
            .iter()
            .filter_map(|tag| Listed::from_tag(tag))
            .map(Language::Known)
    }

    #[must_use]
    pub(crate) fn read(stored: &str) -> Language {
        let stored = stored.trim();
        if stored.is_empty() || stored.eq_ignore_ascii_case(UNDETERMINED) {
            return Language::Auto;
        }
        match Listed::from_tag(stored) {
            Some(listed) => Language::Known(listed),
            None => Language::Custom(stored.to_owned()),
        }
    }

    #[must_use]
    pub(crate) fn stored(&self) -> &str {
        match self {
            Language::Auto => "",
            Language::Known(listed) => listed.tag(),
            Language::Custom(input) => input,
        }
    }

    #[must_use]
    pub(crate) fn label(&self) -> Label {
        match self {
            Language::Auto => tr!(AUTO),
            Language::Known(listed) => Label::from(listed.native_name()),
            Language::Custom(input) => Label::from(input.clone()),
        }
    }

    #[must_use]
    pub(crate) fn named(&self) -> Option<&str> {
        match self {
            Language::Auto => None,
            Language::Known(listed) => Some(listed.english_name()),
            Language::Custom(input) => Some(input),
        }
    }

    #[must_use]
    pub(super) fn is_auto(&self) -> bool {
        *self == Language::Auto
    }

    #[must_use]
    pub(crate) fn session() -> Language {
        Listed::lookup(&current_locale().to_string()).map_or(Language::Auto, Language::Known)
    }
}
