use rikka_app::prelude::*;
use rikka_nix::Module;
use rikka_settings::session::language::{DOMAIN, LANGUAGE};
use rikka_widgets::column;

use crate::domain::{Domain, domain_page};
use crate::language::locales;
use crate::notice::domain_notice;
use crate::page::Page;
use crate::pending::Pending;
use crate::picker::{default_picker, value_picker};
use crate::session::Session;

const LOCALE: &str = "i18n.defaultLocale";
const MESSAGES: &str = "i18n.extraLocaleSettings.LC_MESSAGES";

pub(crate) struct Language {
    domain: Domain,
}

impl Page for Language {
    fn opened(&mut self, _: &Pending, session: &Session) {
        self.domain.open::<Self>(session);
    }
}

domain_page!(Language);

impl Language {
    pub(crate) fn new() -> Self {
        Self {
            domain: Domain::new(DOMAIN),
        }
    }

    fn choose(&mut self, index: Option<usize>) {
        let Some(index) = index else {
            self.domain.reset(LANGUAGE);
            return;
        };
        if let Some(language) = rikka_i18n::languages().get(index) {
            self.domain.set(LANGUAGE, language.tag());
        }
    }

    fn language(&self) -> Option<String> {
        self.domain.non_empty_string(LANGUAGE)
    }
}

pub(crate) fn view(language: &Language, pending: &Pending) -> impl View {
    column! {
        domain_notice(
            &language.domain,
            "settings.language.unavailable",
            "settings.language.refused",
        );
        desktop(language);
        if pending.unavailable().is_none() {
            region(pending);
        }
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn desktop(language: &Language) -> impl View {
    let languages = rikka_i18n::languages();
    section(
        tr!("settings.language.desktop"),
        match languages.is_empty() {
            true => text(tr!("settings.language.untranslated")).label().build(),
            false => field(
                tr!("settings.language.language"),
                picker(language, &languages),
            )
            .build(),
        },
    )
}

fn picker(language: &Language, languages: &[rikka_i18n::Language]) -> impl View {
    let tag = language.language();
    let selected = tag
        .as_deref()
        .and_then(|tag| languages.iter().position(|language| language.tag() == tag));
    default_picker(
        tr!("settings.language.default"),
        languages
            .iter()
            .map(|language| Label::from(language.name())),
        selected,
        |language: &mut Language, index: Option<usize>| language.choose(index),
    )
}

fn region(pending: &Pending) -> impl View {
    let choices = locales::choices();
    section(
        tr!("settings.language.region"),
        column! {
            field(
                tr!("settings.language.locale"),
                value_picker(
                    tr!("settings.language.unset"),
                    choices.clone(),
                    pending.get::<String>(Module::System, LOCALE).as_deref(),
                    |pending: &mut Pending, locale: Option<String>| {
                        pending.write(Module::System, LOCALE, locale.as_deref());
                    },
                ),
            );
            field(
                tr!("settings.language.messages"),
                value_picker(
                    tr!("settings.language.follow-locale"),
                    choices,
                    pending.get::<String>(Module::System, MESSAGES).as_deref(),
                    |pending: &mut Pending, locale: Option<String>| {
                        pending.write(Module::System, MESSAGES, locale.as_deref());
                    },
                ),
            );
        }
        .gap(Space::SNUG),
    )
}

impl std::fmt::Debug for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Language")
            .field("language", &self.language())
            .finish_non_exhaustive()
    }
}
