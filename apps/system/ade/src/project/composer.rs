use rikka_app::prelude::*;
use rikka_chat::{Composer, Offering, QuestionForm, attach, context_fill, outgoing_message};
use rikka_llm::{ModelInfo, Trouble};
use rikka_thread::{Attachment, Job, QueuedAsk, Summary, Thread};
use rikka_widgets::row;

use crate::ade::Ade;
use crate::project::chat::AdeChat;
use crate::project::machine::{self, Machine};
use crate::project::runner::Runner;
use crate::project::terminal::{self, Terminal};
use crate::project::worktree;

const MODELS: f32 = 260.0;

const NAMING: usize = 2;

const SHELL: usize = 3;

pub(super) struct Composing<'a> {
    pub(super) thread: &'a str,
    pub(super) draft: &'a TextDocument,
    pub(super) attached: &'a [Attachment],
    pub(super) model: Option<&'a ModelInfo>,
    pub(super) answering: bool,
    pub(super) asking: Option<&'a QuestionForm>,
    pub(super) offering: Option<&'a Offering>,
    pub(super) error_message: Option<&'a Trouble>,
    pub(super) machine: Option<&'a Machine>,
    pub(super) folded: bool,
    pub(super) jobs: &'a [Job],
    pub(super) watching: Option<u64>,
    pub(super) waiting: &'a [QueuedAsk],
    pub(super) naming: Option<worktree::Naming<'a>>,
    pub(super) panels: Vec<Element>,
    pub(super) terminal: Option<&'a Terminal>,
}

pub(super) fn view(composing: Composing<'_>) -> impl View {
    let screen = composing
        .machine
        .and_then(|machine| machine::view(machine, composing.folded, !composing.answering));
    let mut composer =
        Composer::<AdeChat>::new(composing.draft, component(|ade: &Ade| ade, controls))
            .thread(composing.thread)
            .attached(composing.attached)
            .model(composing.model)
            .asking(composing.asking)
            .offering(composing.offering)
            .error(composing.error_message)
            .jobs(composing.jobs, composing.watching)
            .waiting(composing.waiting)
            .panels(composing.panels);
    if let Some(screen) = screen {
        composer = composer.screen(screen, !composing.folded);
    }
    match (&composing.naming, composing.terminal) {
        (Some(naming), _) => composer = composer.instead(NAMING, worktree::form(naming)),
        (None, Some(terminal)) if terminal.is_shown() => {
            composer = composer.instead(SHELL, terminal::view(terminal));
        }
        _ => {}
    }
    composer.view()
}

fn controls(ade: &Ade) -> Element {
    let (Some(project), setup) = (ade.project(), ade.setup()) else {
        return Element::empty();
    };
    let selection = project.thread().and_then(Thread::selection);
    let runner = project.runner();
    let watching = runner.and_then(Runner::watching).is_some();
    let offers = project.offers();
    let models = project.models(&setup.llm);
    let gauge = match runner.filter(|_| watching).and_then(Runner::hand) {
        Some(hand) => hand
            .model()
            .and_then(|model| context_fill(runner.map_or(0, Runner::shown_filled), model)),
        None => setup.llm.model_of(selection).and_then(|model| {
            context_fill(project.summary().map_or(0, Summary::context_tokens), model)
        }),
    };
    row! {
        attach::<AdeChat>(!watching);
        if !models.is_empty() {
            models.trigger::<AdeChat>(project.picking_models(), MODELS);
        }
        if let Some(choosing) = project.choosing(&offers).as_ref() {
            worktree::picker(choosing);
        }
        spacer().grow(1.0);
        gauge;
        outgoing_message::<AdeChat>(
            runner.is_some_and(Runner::is_shown_running),
            project.draft(),
            !project.attached().is_empty(),
            project.is_sending() || watching,
        );
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .shrink(0.0)
    .fill_width()
    .build()
}
