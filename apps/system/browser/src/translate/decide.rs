use browser_preferences::{Preferences, Rule, TranslationRules};
use browser_translators::{Command, Watch};
use rikka_languages::Language;

use crate::translate::tabs::{Follow, Session};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Page<'a> {
    pub(crate) language: Option<&'a Language>,
    pub(crate) target: &'a Language,
    pub(crate) site: Option<&'a str>,
}

impl Page<'_> {
    fn excluded(&self, rules: &TranslationRules) -> bool {
        self.language.is_some_and(|language| {
            language == self.target || rules.names_language(Rule::NeverLanguage, language)
        }) || self
            .site
            .is_some_and(|site| rules.names_site(Rule::NeverSite, site))
    }
}

pub(crate) fn decide<R>(
    preferences: &Preferences,
    language: Option<Language>,
    address: &str,
    decide: impl FnOnce(Page<'_>, &TranslationRules) -> R,
) -> R {
    let target = preferences.translate_to();
    let site = browser_pages::web_site(address);
    let page = Page {
        language: language.as_ref(),
        target: &target,
        site: site.as_deref(),
    };
    decide(page, &preferences.translation_rules())
}

pub(crate) fn automatic(page: Page<'_>, rules: &TranslationRules, follow: Option<&Follow>) -> bool {
    if page.excluded(rules) {
        return false;
    }
    let followed = follow.is_some_and(|follow| match (&follow.source, page.language) {
        (Some(source), Some(language)) => source == language,
        _ => true,
    });
    followed
        || page
            .language
            .is_some_and(|language| rules.names_language(Rule::AlwaysLanguage, language))
        || page
            .site
            .is_some_and(|site| rules.names_site(Rule::AlwaysSite, site))
}

pub(crate) fn offered(page: Page<'_>, rules: &TranslationRules, offer: bool) -> bool {
    offer && page.language.is_some() && !page.excluded(rules)
}

pub(crate) fn answer(desired: Option<&Session>, frame: &Watch) -> Option<Command> {
    match desired {
        Some(session)
            if frame.session == Some(session.id) && frame.display == Some(session.display) =>
        {
            None
        }
        Some(session) => Some(session.command()),
        None if frame.session.is_none() => None,
        None => Some(Command::Restore),
    }
}
