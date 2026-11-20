use installer_core::{Answers, Disk, Problem, Step, validate, validate_step};
use installer_sys::{Environment, Event};

use crate::{
    form::{Field, Form, Tone},
    glyphs::Glyphs,
    overlay::{Confirm, Decision, Overlay, Picker},
    progress::InstallView,
    screen::{self, Context},
    theme::Theme,
};

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Quit,
    Reboot,
}

#[derive(Debug)]
pub struct App {
    answers: Answers,
    environment: Environment,
    disks: Vec<Disk>,
    timezones: Vec<String>,
    step: Step,
    form: Form,
    overlay: Option<Overlay>,
    install: InstallView,
    theme: Theme,
    glyphs: Glyphs,
    outcome: Option<Outcome>,
    start_requested: bool,
}

impl App {
    #[must_use]
    pub fn new(environment: Environment, disks: Vec<Disk>, timezones: Vec<String>) -> Self {
        let mut answers = Answers {
            release: environment.release.clone(),
            timezone: environment.timezone.clone(),
            gpus: environment.gpus.clone(),
            ..Answers::default()
        };
        if let Ok(catalog) = &environment.catalog {
            answers.offer(catalog.clone());
        }
        answers.nvidia = answers.nvidia_offer().is_some();
        answers.allow_unfree = answers.nvidia;
        let mut app = Self {
            answers,
            environment,
            disks,
            timezones,
            step: Step::Language,
            form: Form::new(Vec::new()),
            overlay: None,
            install: InstallView::default(),
            theme: Theme::for_terminal(&var("COLORTERM"), &var("TERM")),
            glyphs: Glyphs::for_terminal(&var("TERM"), &var("LANG")),
            outcome: None,
            start_requested: false,
        };
        app.refresh();
        app
    }

    #[must_use]
    pub fn plain(mut self) -> Self {
        self.glyphs = Glyphs::ascii();
        self.theme = Theme::console();
        self
    }

    #[must_use]
    pub fn answers(&self) -> &Answers {
        &self.answers
    }

    #[must_use]
    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    #[must_use]
    pub fn step(&self) -> Step {
        self.step
    }

    #[must_use]
    pub fn form(&self) -> &Form {
        &self.form
    }

    #[must_use]
    pub fn overlay(&self) -> Option<&Overlay> {
        self.overlay.as_ref()
    }

    #[must_use]
    pub fn install(&self) -> &InstallView {
        &self.install
    }

    #[must_use]
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    #[must_use]
    pub fn glyphs(&self) -> &Glyphs {
        &self.glyphs
    }

    #[must_use]
    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    #[must_use]
    pub fn problems(&self) -> Vec<Problem> {
        validate_step(&self.answers, self.step)
    }

    #[must_use]
    pub fn all_problems(&self) -> Vec<Problem> {
        validate(&self.answers)
    }

    #[must_use]
    pub fn detail(&self) -> Vec<(String, Tone)> {
        screen::detail(self.step, &self.answers)
    }

    pub fn apply(&mut self, event: Event) {
        self.install.apply(event);
    }

    pub fn tick(&mut self) {
        self.install.tick();
    }

    pub fn take_start_request(&mut self) -> bool {
        std::mem::take(&mut self.start_requested)
    }

    pub(crate) fn quit(&mut self, outcome: Outcome) {
        self.outcome = Some(outcome);
    }

    pub(crate) fn install_mut(&mut self) -> &mut InstallView {
        &mut self.install
    }

    pub(crate) fn form_mut(&mut self) -> &mut Form {
        &mut self.form
    }

    pub(crate) fn show(&mut self, overlay: Overlay) {
        self.overlay = Some(overlay);
    }

    pub(crate) fn dismiss(&mut self) {
        self.overlay = None;
    }

    pub(crate) fn picker_mut(&mut self) -> Option<&mut Picker> {
        match &mut self.overlay {
            Some(Overlay::Picker(picker)) => Some(picker),
            _ => None,
        }
    }

    pub(crate) fn confirm_mut(&mut self) -> Option<&mut Confirm> {
        match &mut self.overlay {
            Some(Overlay::Confirm(confirm)) => Some(confirm),
            _ => None,
        }
    }

    pub(crate) fn commit(&mut self) {
        screen::commit(
            self.step,
            &self.form,
            &mut self.answers,
            &self.disks,
            &self.timezones,
        );
    }

    pub(crate) fn refresh(&mut self) {
        self.commit();

        let rebuilt = self.rebuild();
        if rebuilt.shape() != self.form.shape() {
            let focus = self.form.focus();
            self.form = rebuilt;
            self.form.set_focus(focus);
        }
    }

    fn rebuild(&self) -> Form {
        screen::build(
            self.step,
            Context {
                answers: &self.answers,
                disks: &self.disks,
                timezones: &self.timezones,
            },
        )
    }

    pub(crate) fn open_picker(&mut self) {
        let Some(field) = self.form.focused() else {
            return;
        };
        let Field::Picker {
            id,
            label,
            options,
            index,
            ..
        } = field
        else {
            return;
        };
        self.show(Overlay::Picker(Picker::new(
            *id,
            *label,
            options.clone(),
            *index,
        )));
    }

    pub(crate) fn advance(&mut self) {
        self.refresh();
        if !self.problems().is_empty() {
            return;
        }

        if self.step == Step::Summary {
            let confirm = Confirm::new(
                "Begin the install",
                self.destruction(),
                Decision::BeginInstall,
            );
            self.show(Overlay::Confirm(confirm));
            return;
        }

        if let Some(next) = self.step.next() {
            self.enter(next);
        }
    }

    pub(crate) fn retreat(&mut self) {
        if self.step == Step::POINT_OF_NO_RETURN {
            return;
        }
        self.refresh();
        if let Some(previous) = self.step.previous() {
            self.enter(previous);
        }
    }

    pub(crate) fn begin_install(&mut self) {
        self.enter(Step::Install);
        self.start_requested = true;
    }

    fn enter(&mut self, step: Step) {
        self.step = step;
        self.form = self.rebuild();
        self.commit();
    }

    fn destruction(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(disk) = &self.answers.disk {
            lines.push(format!("{} will be repartitioned.", disk.path));
            lines.push(format!("Everything on {} is lost.", disk.describe()));
        }
        if !self.environment.mode.is_live() {
            lines.push(String::new());
            lines.push("Nothing is run in this mode; commands are only shown.".to_owned());
        }
        lines
    }
}
