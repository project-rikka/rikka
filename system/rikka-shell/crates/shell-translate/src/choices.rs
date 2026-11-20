use std::cell::RefCell;
use std::rc::Rc;

use rikka_llm::Effort;
use rikka_llm_accounts::{AccountId, Selection};
use rikka_seam::SeamClient;
use rikka_settings::seam::protocol;
use rikka_settings::session::translate;
use rikka_types::Choice;
use shell_kit::Shell;

use crate::language::Language;
use crate::profile::Profile;

const FALLBACK: &str = "en";

#[derive(Clone, Debug, Default)]
struct Values {
    target: String,
    source: String,
    profile: Profile,
    backend: String,
    model: String,
    effort: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Choices(Rc<RefCell<Values>>);

impl Choices {
    #[must_use]
    pub(crate) fn target(&self) -> Language {
        let stored = self.0.borrow().target.clone();
        match Language::read(&stored) {
            Language::Auto => session(),
            chosen => chosen,
        }
    }

    #[must_use]
    pub(crate) fn source(&self) -> Language {
        Language::read(&self.0.borrow().source)
    }

    #[must_use]
    pub(crate) fn profile(&self) -> Profile {
        self.0.borrow().profile
    }

    #[must_use]
    pub(crate) fn chosen(&self, otherwise: Effort) -> Option<Selection> {
        let values = self.0.borrow();
        let account = AccountId::from_id(&values.backend)?;
        if values.model.is_empty() {
            return None;
        }
        let effort = Effort::from_id(&values.effort).unwrap_or(otherwise);
        Some(Selection::new(account, values.model.clone(), effort))
    }

    pub(super) fn choose_target(&self, seam: &SeamClient, target: &Language) {
        self.write(seam, translate::TARGET, target.stored(), |values, input| {
            values.target = input;
        });
    }

    pub(super) fn choose_source(&self, seam: &SeamClient, source: &Language) {
        self.write(seam, translate::SOURCE, source.stored(), |values, input| {
            values.source = input;
        });
    }

    pub(super) fn choose_profile(&self, seam: &SeamClient, profile: Profile) {
        self.write(seam, translate::PROFILE, profile.slug(), |values, _| {
            values.profile = profile
        });
    }

    pub(super) fn choose_model(&self, seam: &SeamClient, selection: &Selection) {
        self.write(
            seam,
            translate::BACKEND,
            &selection.account().to_string(),
            |values, input| values.backend = input,
        );
        self.write(
            seam,
            translate::MODEL,
            selection.model(),
            |values, input| values.model = input,
        );
        self.write(
            seam,
            translate::EFFORT,
            selection.effort().id(),
            |values, input| values.effort = input,
        );
    }

    fn write(
        &self,
        seam: &SeamClient,
        key: &str,
        value: &str,
        apply: impl FnOnce(&mut Values, String),
    ) {
        apply(&mut self.0.borrow_mut(), value.to_owned());
        seam.notify(protocol::set(translate::DOMAIN, key, value));
    }

    fn hold(&self, apply: impl FnOnce(&mut Values, String), value: String) {
        apply(&mut self.0.borrow_mut(), value);
    }
}

pub(crate) fn follow(shell: Shell) -> (Shell, Choices) {
    let choices = Choices::default();

    let shell = watch(shell, &choices, translate::TARGET, |values, input| {
        values.target = input;
    });
    let shell = watch(shell, &choices, translate::SOURCE, |values, input| {
        values.source = input;
    });
    let shell = watch(shell, &choices, translate::PROFILE, |values, input| {
        values.profile = Profile::of_slug(&input);
    });
    let shell = watch(shell, &choices, translate::BACKEND, |values, input| {
        values.backend = input;
    });
    let shell = watch(shell, &choices, translate::MODEL, |values, input| {
        values.model = input;
    });
    let shell = watch(shell, &choices, translate::EFFORT, |values, input| {
        values.effort = input;
    });

    (shell, choices)
}

fn watch(
    shell: Shell,
    choices: &Choices,
    key: &'static str,
    apply: fn(&mut Values, String),
) -> Shell {
    let choices = choices.clone();
    shell.follow(translate::DOMAIN, key, move |_, input: String| {
        choices.hold(apply, input);
    })
}

fn session() -> Language {
    match Language::session() {
        Language::Auto => Language::read(FALLBACK),
        chosen => chosen,
    }
}
