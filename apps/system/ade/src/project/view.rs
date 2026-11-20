use rikka_app::prelude::*;
use rikka_chat::{Transcript, empty_state, transcript};
use rikka_thread::Thread;
use rikka_types::{Key, Modifiers};
use rikka_ui::search::{Action, Step};
use rikka_widgets::{column, find_bar, find_mark, row};

use crate::ade::Ade;
use crate::labels;
use crate::project::act::act;
use crate::project::chat::AdeChat;
use crate::project::runner::Runner;
use crate::project::state::Project;
use crate::project::tab::Tab;
use crate::project::terminal::Terminal;
use crate::project::{composer, file, git, palette, sidebar};

const FIND: &str = "ade-conversation-find";
const PALETTE: &str = "ade-palette-mark";
const PALETTE_SHOWN: &str = "ade-palette";
const PALETTE_INSET: f32 = 24.0;
const PALETTE_DROP: f32 = 72.0;

pub(crate) fn view() -> impl View {
    let body = column! {
        row! {
            spacer()
                .width(palette::WIDTH + PALETTE_INSET)
                .height(0.0)
                .build()
                .keyed(PALETTE);
        }
        .justify(Justify::Center)
        .fill_width();
        row! {
            component(|project: &Project| project, sidebar::view);
            rule_upright();
            component(|project: &Project| project, work);
        }
        .grow(1.0)
        .fill_width();
        component(|project: &Project| project, summoned_palette);
    }
    .fill();
    container(body)
        .on_chord(
            Key::Char('p'),
            Modifiers::CONTROL,
            act(|project| project.summon_palette()),
        )
        .on_chord(
            Key::Char('e'),
            Modifiers::CONTROL | Modifiers::SHIFT,
            act(|project| project.turn_to(Tab::Files)),
        )
        .fill()
}

fn summoned_palette(project: &Project) -> Option<Element> {
    let summoned = project.palette()?;
    Some(
        palette::view(summoned)
            .keyed(PALETTE_SHOWN)
            .anchored(Anchor::new(PALETTE, AnchorEdge::BelowStart).offset(0.0, PALETTE_DROP)),
    )
}

pub(crate) fn heading() -> Element {
    component(
        |project: &Project| project,
        |project: &Project| {
            project
                .heading()
                .map(|heading| text(heading).label().muted().no_wrap())
        },
    )
}

pub(crate) fn place() -> Element {
    component(
        |project: &Project| project,
        |project: &Project| {
            project.place().map(|worktree| {
                row! {
                    icon(MaterialSymbol::CHEVRON_RIGHT)
                        .extent(Length::px(14.0))
                        .color(palette().text_muted);
                    text(worktree).label().muted().no_wrap();
                }
                .gap(Space::SNUG)
                .align_items(Align::Center)
            })
        },
    )
}

fn work(project: &Project) -> impl View {
    let tab = project.tab();
    let editing = tab == Tab::Files && project.shown().viewer().shown().is_some();
    let (page, pane) = match (tab, editing) {
        (Tab::Git, _) => (2usize, component(|project: &Project| project, change)),
        (_, true) => (1, component(|project: &Project| project, file_pane)),
        _ => (0, component(|project: &Project| project, thread_pane)),
    };
    page_stack(page, pane).grow(1.0).fill_height()
}

fn change(project: &Project) -> impl View {
    let _minute = project.minute();
    git::change(project.shown().git(), project.places())
}

fn file_pane(project: &Project) -> Element {
    let workspace = project.shown();
    let Some(shown) = workspace.viewer().shown() else {
        return Element::empty();
    };
    let mut pane = container(file::page(
        workspace.viewer(),
        workspace.root(),
        workspace.git(),
    ))
    .on_command(
        Command::Close,
        act(|project| project.viewer(|viewer, git| viewer.close_active(git))),
    )
    .on_command(
        Command::NextTab,
        act(|project| project.viewer(|viewer, git| viewer.cycle(1, git))),
    )
    .on_command(
        Command::PreviousTab,
        act(|project| project.viewer(|viewer, git| viewer.cycle(-1, git))),
    )
    .on_command(
        Command::Save,
        act(|project| project.viewer(|viewer, _| viewer.save())),
    )
    .on_command(
        Command::Find,
        act(|project| project.viewer(|viewer, _| viewer.open_find(false))),
    )
    .on_command(
        Command::FindNext,
        act(|project| project.viewer(|viewer, _| viewer.find(Action::Step(Step::Next)))),
    )
    .on_command(
        Command::FindPrevious,
        act(|project| {
            project.viewer(|viewer, _| viewer.find(Action::Step(Step::Previous)));
        }),
    )
    .on_chord(
        Key::Char('h'),
        Modifiers::CONTROL,
        act(|project| project.viewer(|viewer, _| viewer.open_find(true))),
    );
    if shown.is_finding() {
        pane = pane.on_chord(
            Key::Escape,
            Modifiers::NONE,
            act(|project| project.viewer(|viewer, _| viewer.find(Action::Close))),
        );
    }
    pane.grow(1.0).fill_height().any()
}

fn thread_pane(project: &Project) -> impl View {
    let finding = project.transcript().finding();
    let work = column! {
        find_mark(FIND);
        component(|project: &Project| project, conversation);
        component(|ade: &Ade| ade, composing);
        find_bar(finding.search())
            .anchored_to(FIND)
            .on_action(|transcript: &mut Transcript, action: Action| transcript.find(action));
    }
    .fill_height()
    .fill_width();

    let mut pane = container(work)
        .on_chord(
            Key::Char('t'),
            Modifiers::CONTROL,
            act(|project| project.raise_terminal(Terminal::toggle)),
        )
        .on_chord(
            Key::Char('t'),
            Modifiers::CONTROL | Modifiers::SHIFT,
            act(|project| project.raise_terminal(Terminal::fresh)),
        )
        .on_command(Command::Find, act(|project| project.open_find()))
        .on_command(
            Command::FindNext,
            act(|project| project.find(Action::Step(Step::Next))),
        )
        .on_command(
            Command::FindPrevious,
            act(|project| project.find(Action::Step(Step::Previous))),
        );
    if finding.is_open() {
        pane = pane.on_chord(
            Key::Escape,
            Modifiers::NONE,
            act(|project| project.find(Action::Close)),
        );
    }
    pane.grow(1.0).fill_height()
}

fn conversation(project: &Project) -> Element {
    let shown = project.transcript();
    transcript::<AdeChat, _>(shown, || match project.is_reading() {
        false => empty_state(tr!(
            "ade.chat.start",
            project = &labels::file_name(&project.shown_root())
        ))
        .any(),
        true => container(spacer()).grow(1.0).fill_width().any(),
    })
}

fn composing(ade: &Ade) -> Element {
    let (Some(project), setup) = (ade.project(), ade.setup()) else {
        return Element::empty();
    };
    let runner = project.runner();
    let picking = project.picking_models();
    let panels = match picking.is_open() {
        true => project.models(&setup.llm).panels::<AdeChat>(picking),
        false => Vec::new(),
    };
    composer::view(composer::Composing {
        thread: project.open_thread(),
        draft: project.draft(),
        attached: project.attached(),
        model: setup
            .llm
            .model_of(project.thread().and_then(Thread::selection)),
        answering: runner.is_some_and(Runner::is_running),
        asking: runner.and_then(Runner::asking),
        offering: project.offering(),
        error_message: project.error_message(),
        machine: runner.map(Runner::machine),
        folded: project.is_folded(),
        jobs: runner.map(Runner::jobs).unwrap_or_default(),
        watching: runner.and_then(Runner::watching),
        waiting: runner.map(Runner::waiting).unwrap_or_default(),
        naming: project.naming(),
        panels,
        terminal: project.shell(),
    })
    .any()
}
