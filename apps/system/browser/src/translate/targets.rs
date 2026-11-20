use browser_translators::Engine;
use rikka_languages::Language;

use crate::preferences::session_language;

const MAJOR: [&str; 20] = [
    "en", "ja", "zh-Hans", "zh-Hant", "ko", "es", "fr", "de", "it", "pt", "ru", "ar", "hi", "id",
    "th", "vi", "tr", "nl", "pl", "uk",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Targets {
    pub(crate) languages: Vec<Language>,
    pub(crate) major: usize,
}

impl Targets {
    pub(crate) fn endonyms(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.languages.iter().map(|language| language.native_name())
    }

    pub(crate) fn position(&self, language: &Language) -> Option<usize> {
        self.languages.iter().position(|listed| listed == language)
    }
}

pub(crate) fn targets(engine: Engine) -> Targets {
    ordered(session_language(), engine)
}

fn ordered(session: Language, engine: Engine) -> Targets {
    let mut languages: Vec<Language> = MAJOR
        .iter()
        .filter_map(|tag| Language::from_tag(tag))
        .filter(|language| engine.serves(language))
        .collect();
    if !languages.contains(&session) && engine.serves(&session) {
        languages.insert(0, session);
    }
    let major = languages.len();
    let rest: Vec<Language> = Language::all()
        .filter(|language| engine.serves(language) && !languages.contains(language))
        .collect();
    languages.extend(rest);
    Targets { languages, major }
}
