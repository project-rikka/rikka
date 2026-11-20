use rikka_agent::CancelToken;
use rikka_llm_accounts::{AccountsChanged, Llm, ModelOptions, Selection};
use rikka_seam::Request;
use rikka_session::following;
use rikka_ui::{State, TextDocument, skip_view, spawn, spawn_blocking, stream};
use shell_capture::{Snapshot, Taken};
use shell_kit::{CLOSE, Spot, copy_to_clipboard};

use crate::error::{Error, Result};
use crate::language::Language;
use crate::picker::{self, Choice};
use crate::profile::Profile;
use crate::service::{CAPABILITY, SNIPPED};
use crate::shared::Shared;
use crate::side::Side;
use crate::translator::{Job, Rendered, Subject, TranslationRequest};

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingTranslation {
    text: Option<String>,
    source: Language,
    target: Language,
    profile: Option<Profile>,
    chosen: Option<Selection>,
}

#[derive(Debug)]
pub(super) struct LanguageInput {
    pub(crate) side: Side,
    pub(crate) typed: TextDocument,
}

#[derive(State)]
pub(crate) struct Translate {
    shared: Shared,
    llm: Option<Llm>,
    input: TextDocument,
    characters: usize,
    translation: TextDocument,
    picture: Option<Snapshot>,
    read: TextDocument,
    source: Language,
    target: Language,
    profile: Profile,
    detected: Option<Language>,
    answered: Option<PendingTranslation>,
    language_input: Option<LanguageInput>,
    trouble: Option<Error>,
    cancel: Option<CancelToken>,
    generation: u64,
    picking: bool,
}

impl Translate {
    #[must_use]
    pub(crate) fn new(shared: Shared) -> Translate {
        let source = shared.choices.source();
        let target = shared.choices.target();
        let profile = shared.choices.profile();
        Translate {
            shared,
            llm: None,
            input: TextDocument::new(""),
            characters: 0,
            translation: TextDocument::new(""),
            picture: None,
            read: TextDocument::new(""),
            source,
            target,
            profile,
            detected: None,
            answered: None,
            language_input: None,
            trouble: None,
            cancel: None,
            generation: 0,
            picking: false,
        }
    }

    pub(crate) fn input(&self) -> &TextDocument {
        &self.input
    }

    pub(crate) fn translation(&self) -> &TextDocument {
        &self.translation
    }

    pub(crate) fn picture(&self) -> Option<&Snapshot> {
        self.picture.as_ref()
    }

    pub(crate) fn read(&self) -> &TextDocument {
        &self.read
    }

    pub(crate) fn source(&self) -> &Language {
        &self.source
    }

    pub(crate) fn target(&self) -> &Language {
        &self.target
    }

    pub(crate) fn profile(&self) -> Profile {
        self.profile
    }

    pub(crate) fn language_input(&self) -> Option<&LanguageInput> {
        self.language_input.as_ref()
    }

    pub(crate) fn trouble(&self) -> Option<&Error> {
        self.trouble.as_ref()
    }

    pub(crate) fn is_working(&self) -> bool {
        self.cancel.is_some()
    }

    pub(super) fn is_picking(&self) -> bool {
        self.picking
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.picture.is_none() && self.characters == 0
    }

    pub(super) fn can_translate(&self) -> bool {
        let within = self.picture.is_some()
            || self
                .most_characters()
                .is_none_or(|most| self.characters <= most);
        self.llm.is_some() && !self.is_blank() && within
    }

    pub(crate) fn is_stale(&self) -> bool {
        !self.translation.is_empty() && self.answered.as_ref() != Some(&self.asking())
    }

    fn is_blank(&self) -> bool {
        self.picture.is_none() && self.input.text().trim().is_empty()
    }

    pub(super) fn characters(&self) -> usize {
        self.characters
    }

    pub(crate) fn spot(&self) -> &Spot {
        &self.shared.spot
    }

    pub(crate) fn picks(&self) -> ModelOptions {
        self.llm
            .as_ref()
            .map(|llm| ModelOptions::new(llm, self.chosen().as_ref()))
            .unwrap_or_default()
    }

    pub(crate) fn chosen(&self) -> Option<Selection> {
        let llm = self.llm.as_ref()?;
        self.shared.choices.chosen(llm.effort())
    }

    pub(super) fn most_characters(&self) -> Option<usize> {
        self.shared.prompts.max_characters().ok()
    }

    pub(super) fn swappable(&self) -> bool {
        picker::swapped(&self.source, &self.target, self.detected.as_ref()).is_some()
    }

    pub(crate) fn start(&mut self) {
        let snipped = following::changes(&self.shared.seam, CAPABILITY, SNIPPED);
        stream(snipped, |translate: &mut Translate, ()| translate.snipped());
        spawn_blocking(Llm::load, Translate::loaded);
    }

    fn loaded(&mut self, llm: Llm) {
        if let Some(announcements) = llm.changes() {
            stream(announcements, Translate::accounts_changed);
        }
        self.llm = Some(llm);
        self.take_snip();
    }

    fn accounts_changed(&mut self, announced: AccountsChanged) {
        if let Some(llm) = self.llm.as_mut() {
            llm.apply_change(announced);
        }
        if self.picks().is_empty() {
            self.picking = false;
        }
    }

    pub(crate) fn typed(&mut self) {
        self.characters = self.input.text().chars().count();
        self.trouble = None;
        if self.is_blank() {
            self.forget();
        }
    }

    pub(crate) fn translate(&mut self) {
        if self.can_translate() {
            self.ask();
        }
    }

    fn ask(&mut self) {
        self.stop();
        let mark = self.generation;

        let Some(subject) = self.subject() else {
            return;
        };
        let Some(llm) = self.llm.as_ref() else {
            return;
        };

        let cancel = CancelToken::new();
        let job = match self.prepare(llm, subject, cancel.clone()) {
            Ok(job) => job,
            Err(error) => {
                self.trouble = Some(error);
                return;
            }
        };

        let asking = self.asking();
        self.cancel = Some(cancel);
        self.trouble = None;
        spawn(
            job.run(),
            move |translate: &mut Translate, answered: Result<Rendered>| {
                translate.answered(mark, &asking, answered);
            },
        );
    }

    fn asking(&self) -> PendingTranslation {
        PendingTranslation {
            text: self.picture.is_none().then(|| self.input.text()),
            source: self.source.clone(),
            target: self.target.clone(),
            profile: self.picture.is_none().then_some(self.profile),
            chosen: self.chosen(),
        }
    }

    fn prepare(&self, llm: &Llm, subject: Subject, cancel: CancelToken) -> Result<Job> {
        let chosen = self.chosen();
        Job::prepare(TranslationRequest {
            llm,
            prompts: &self.shared.prompts,
            chosen: chosen.as_ref(),
            target: &self.target,
            source: &self.source,
            profile: self.profile,
            subject,
            cancel,
        })
    }

    fn subject(&self) -> Option<Subject> {
        if let Some(picture) = &self.picture {
            return Some(Subject::Picture(picture.png.clone()));
        }
        let input = self.input.text();
        (!input.trim().is_empty()).then_some(Subject::Text(input))
    }

    fn answered(&mut self, mark: u64, asking: &PendingTranslation, answered: Result<Rendered>) {
        if mark != self.generation {
            return skip_view();
        }
        self.cancel = None;
        match answered {
            Ok(rendered) => {
                self.translation.set_text(&rendered.translation);
                self.read.set_text(rendered.original.unwrap_or_default());
                self.detected = Some(rendered.source).filter(|language| !language.is_auto());
                self.answered = Some(asking.clone());
                self.trouble = None;
            }
            Err(error) => self.trouble = Some(error),
        }
    }

    pub(crate) fn chose(&mut self, side: Side, index: usize) {
        let current = self.side(side).clone();
        match picker::offered(side, &current).get(index) {
            Some(Choice::Speaking(chosen)) => self.take(side, chosen.clone()),
            Some(Choice::Other) | None => self.name(side),
        }
    }

    fn name(&mut self, side: Side) {
        let input = match self.side(side) {
            Language::Custom(input) => input.clone(),
            _ => String::new(),
        };
        self.language_input = Some(LanguageInput {
            side,
            typed: TextDocument::new(input),
        });
    }

    pub(crate) fn named(&mut self) {
        let Some(language_input) = self.language_input.take() else {
            return;
        };
        let input = language_input.typed.text().trim().to_owned();
        if input.is_empty() {
            return;
        }
        self.take(language_input.side, Language::read(&input));
    }

    fn take(&mut self, side: Side, chosen: Language) {
        if *self.side(side) == chosen {
            return;
        }
        match side {
            Side::Source => {
                self.source = chosen;
                self.shared
                    .choices
                    .choose_source(&self.shared.seam, &self.source);
            }
            Side::Target => {
                self.target = chosen;
                self.shared
                    .choices
                    .choose_target(&self.shared.seam, &self.target);
            }
        }
        self.trouble = None;
    }

    pub(super) fn choose_profile(&mut self, index: usize) {
        let Some(chosen) = Profile::listed(index) else {
            return;
        };
        if chosen == self.profile {
            return;
        }
        self.profile = chosen;
        self.shared
            .choices
            .choose_profile(&self.shared.seam, chosen);
        self.trouble = None;
    }

    pub(crate) fn swap(&mut self) {
        let Some((source, target)) =
            picker::swapped(&self.source, &self.target, self.detected.as_ref())
        else {
            return;
        };
        self.stop();
        let fresh = !self.is_stale();
        self.source = source;
        self.target = target;
        self.shared
            .choices
            .choose_source(&self.shared.seam, &self.source);
        self.shared
            .choices
            .choose_target(&self.shared.seam, &self.target);

        let translation = self.translation.text();
        if self.picture.is_none() && !translation.trim().is_empty() {
            let input = self.input.text();
            self.write(&translation);
            self.translation.set_text(&input);
            if fresh {
                self.answered = Some(self.asking());
            }
        }
        self.detected = None;
        self.trouble = None;
    }

    pub(crate) fn model(&mut self, row: usize) {
        self.picking = false;
        let selection = self.picks().selection_for_row(row);
        self.choose(selection)
    }

    pub(crate) fn effort(&mut self, index: usize) {
        let selection = self.picks().selection_with_effort(index);
        self.choose(selection)
    }

    pub(crate) fn pick(&mut self, open: bool) {
        self.picking = open;
    }

    fn choose(&mut self, picked: Option<Selection>) {
        let Some(picked) = picked else {
            return;
        };
        self.shared.choices.choose_model(&self.shared.seam, &picked);
        self.trouble = None;
    }

    fn snipped(&mut self) {
        if self.llm.is_none() || !self.take_snip() {
            skip_view();
        }
    }

    fn take_snip(&mut self) -> bool {
        match self.shared.snip.take() {
            Some(Taken::Snipped(snapshot)) => {
                self.picture = Some(snapshot);
                self.write("");
                self.forget();
                self.ask();
                true
            }
            Some(Taken::Failed(refused)) => {
                self.trouble = Some(Error::Picture(refused.to_string()));
                true
            }
            None => false,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.picture = None;
        self.write("");
        self.forget();
    }

    fn forget(&mut self) {
        self.stop();
        self.translation.set_text("");
        self.read.set_text("");
        self.detected = None;
        self.answered = None;
        self.trouble = None;
    }

    pub(crate) fn copy(&self) {
        copy_to_clipboard(&self.translation.text());
    }

    pub(crate) fn dismiss(&mut self) {
        match self.language_input.is_some() {
            true => self.language_input = None,
            false => self.close(),
        }
    }

    pub(crate) fn leave(&mut self) {
        self.stop();
    }

    fn write(&mut self, text: &str) {
        self.input.set_text(text);
        self.characters = text.chars().count();
    }

    pub(crate) fn stop(&mut self) {
        self.generation += 1;
        if let Some(cancel) = self.cancel.take() {
            cancel.cancel();
        }
    }

    fn side(&self, side: Side) -> &Language {
        match side {
            Side::Source => &self.source,
            Side::Target => &self.target,
        }
    }

    fn close(&mut self) {
        self.stop();
        self.shared.seam.notify(Request::new(CAPABILITY, CLOSE));
    }
}

impl std::fmt::Debug for Translate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Translate")
            .field("source", &self.source)
            .field("target", &self.target)
            .field("picture", &self.picture.is_some())
            .field("working", &self.is_working())
            .finish_non_exhaustive()
    }
}
