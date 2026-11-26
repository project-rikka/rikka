use browser_pages::{covers, site_of};
use browser_translators::{CodeMode, Display, Engine};
use rikka_languages::Language;
use rikka_settings::{Key, Schema};

use crate::Preferences;

const ENGLISH: &str = "en";
const TARGET: &str = "translate-target";
const ENGINE: &str = "translate-engine";
const DISPLAY: &str = "translate-display";
const CODE: &str = "translate-code";
const PROTECT: &str = "translate-protect";
const OFFER: &str = "translate-offer";
const ALWAYS_LANGUAGES: &str = "translate-always-languages";
const NEVER_LANGUAGES: &str = "translate-never-languages";
const ALWAYS_SITES: &str = "translate-always-sites";
const NEVER_SITES: &str = "translate-never-sites";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rule {
    AlwaysLanguage,
    NeverLanguage,
    AlwaysSite,
    NeverSite,
}

impl Rule {
    pub const ALL: [Rule; 4] = [
        Rule::AlwaysLanguage,
        Rule::NeverLanguage,
        Rule::AlwaysSite,
        Rule::NeverSite,
    ];

    fn key(self) -> &'static str {
        match self {
            Rule::AlwaysLanguage => ALWAYS_LANGUAGES,
            Rule::NeverLanguage => NEVER_LANGUAGES,
            Rule::AlwaysSite => ALWAYS_SITES,
            Rule::NeverSite => NEVER_SITES,
        }
    }

    fn opposite(self) -> Rule {
        match self {
            Rule::AlwaysLanguage => Rule::NeverLanguage,
            Rule::NeverLanguage => Rule::AlwaysLanguage,
            Rule::AlwaysSite => Rule::NeverSite,
            Rule::NeverSite => Rule::AlwaysSite,
        }
    }

    fn names_sites(self) -> bool {
        matches!(self, Rule::AlwaysSite | Rule::NeverSite)
    }

    fn entry(self, value: &str) -> Option<String> {
        let entry = match self.names_sites() {
            true => site_of(value),
            false => Language::lookup(value)?.tag().to_owned(),
        };
        (!entry.is_empty()).then_some(entry)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TranslationRules {
    pub always_languages: Vec<String>,
    pub never_languages: Vec<String>,
    pub always_sites: Vec<String>,
    pub never_sites: Vec<String>,
}

impl TranslationRules {
    pub fn list(&self, rule: Rule) -> &[String] {
        match rule {
            Rule::AlwaysLanguage => &self.always_languages,
            Rule::NeverLanguage => &self.never_languages,
            Rule::AlwaysSite => &self.always_sites,
            Rule::NeverSite => &self.never_sites,
        }
    }

    pub fn names_language(&self, rule: Rule, language: &Language) -> bool {
        self.list(rule)
            .iter()
            .any(|tag| Language::lookup(tag).as_ref() == Some(language))
    }

    pub fn names_site(&self, rule: Rule, site: &str) -> bool {
        self.list(rule).iter().any(|rule| covers(rule, site))
    }
}

impl Preferences {
    pub fn translate_target(&self) -> Option<Language> {
        self.observers.read();
        Language::lookup(&self.settings.borrow().get::<String>(TARGET))
    }

    pub fn set_translate_target(&self, target: Option<&Language>) {
        let code = target.map(|language| language.tag().to_owned());
        self.put::<String>(TARGET, code.unwrap_or_default());
    }

    pub fn translate_to(&self) -> Language {
        self.translate_target().unwrap_or_else(session_language)
    }

    pub fn translate_engine(&self) -> Engine {
        self.observers.read();
        Engine::from_id(&self.settings.borrow().get::<String>(ENGINE)).unwrap_or_default()
    }

    pub fn set_translate_engine(&self, engine: Engine) {
        self.put::<String>(ENGINE, engine.id().to_owned());
    }

    pub fn translate_display(&self) -> Display {
        self.observers.read();
        self.settings.borrow().choice(DISPLAY)
    }

    pub fn set_translate_display(&self, display: Display) {
        self.write(|settings| settings.set_choice(DISPLAY, display));
    }

    pub fn translate_code(&self) -> CodeMode {
        self.observers.read();
        self.settings.borrow().choice(CODE)
    }

    pub fn set_translate_code(&self, code: CodeMode) {
        self.write(|settings| settings.set_choice(CODE, code));
    }

    pub fn translate_protect(&self) -> bool {
        self.observers.read();
        self.settings.borrow().get(PROTECT)
    }

    pub fn set_translate_protect(&self, on: bool) {
        self.put::<bool>(PROTECT, on);
    }

    pub fn translate_offer(&self) -> bool {
        self.observers.read();
        self.settings.borrow().get(OFFER)
    }

    pub fn set_translate_offer(&self, on: bool) {
        self.put::<bool>(OFFER, on);
    }

    pub fn translation_rules(&self) -> TranslationRules {
        self.observers.read();
        let settings = self.settings.borrow();
        TranslationRules {
            always_languages: settings.get(ALWAYS_LANGUAGES),
            never_languages: settings.get(NEVER_LANGUAGES),
            always_sites: settings.get(ALWAYS_SITES),
            never_sites: settings.get(NEVER_SITES),
        }
    }

    pub fn set_translation_rule(&self, rule: Rule, value: &str, on: bool) {
        let Some(entry) = rule.entry(value) else {
            return;
        };
        let rules = self.translation_rules();
        let mut listed = rules.list(rule).to_vec();
        listed.retain(|candidate| *candidate != entry);
        if on {
            listed.push(entry.clone());
            listed.sort();
            let mut opposite = rules.list(rule.opposite()).to_vec();
            if opposite.contains(&entry) {
                opposite.retain(|candidate| *candidate != entry);
                self.put::<Vec<String>>(rule.opposite().key(), opposite);
            }
        }
        self.put::<Vec<String>>(rule.key(), listed);
    }
}

pub fn session_language() -> Language {
    Language::lookup(&rikka_i18n::current_locale().to_string())
        .or_else(|| Language::from_tag(ENGLISH))
        .expect("language table lists English")
}

pub(super) fn keys(schema: Schema) -> Schema {
    schema
        .key(
            Key::string(TARGET, "").summary("Translate pages into this language, or the session's"),
        )
        .key(
            Key::string(ENGINE, Engine::default().id())
                .choices(Engine::all().map(Engine::id))
                .summary("Translation service"),
        )
        .key(Key::choice::<Display>(DISPLAY).summary("Replace the text or show both languages"))
        .key(
            Key::choice::<CodeMode>(CODE)
                .summary("Translate only the comments in code, or skip code"),
        )
        .key(Key::bool(PROTECT, true).summary("Keep code identifiers in prose untranslated"))
        .key(Key::bool(OFFER, true).summary("Offer to translate pages in other languages"))
        .key(Key::list(ALWAYS_LANGUAGES, []).summary("Languages always translated"))
        .key(Key::list(NEVER_LANGUAGES, []).summary("Languages never translated"))
        .key(Key::list(ALWAYS_SITES, []).summary("Sites always translated"))
        .key(Key::list(NEVER_SITES, []).summary("Sites never translated"))
}
