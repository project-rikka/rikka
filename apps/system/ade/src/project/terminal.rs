use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rikka_app::prelude::*;
use rikka_terminal::{Config, Font, Question, Report, Session, screen};
use rikka_types::{Key, Modifiers};
use rikka_ui::search::{Action, Search};
use rikka_widgets::{column, find_bar, find_mark, row};

use crate::notice::notice;
use crate::project::ARRIVAL;
use crate::project::act::{act, given};
use crate::project::handle::ProjectHandle;
use crate::project::state::Project;

const TALL: f32 = 320.0;

const GLYPH: f32 = 14.0;
const ACTION: f32 = 18.0;

const GRID: &str = "ade-terminal";

const FIND: &str = "ade-terminal-find";

static OPENED: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Chord {
    Hide,
    Fresh,
    OpenFind,
}

#[derive(Debug)]
struct Shell {
    serial: u64,
    at: PathBuf,
    session: Rc<Session>,
    font: Font,
}

#[derive(Debug)]
enum Started {
    Nothing,
    Running(Shell),
    Failed(Arc<str>),
}

#[derive(Debug, State)]
pub(crate) struct Terminal {
    #[untracked]
    handle: ProjectHandle,
    started: Tracked<Started>,
    title: Tracked<Option<String>>,
    shown: Tracked<bool>,
    #[untracked]
    search: Search,
}

impl Terminal {
    pub(crate) fn new(handle: &ProjectHandle) -> Terminal {
        Terminal {
            handle: handle.clone(),
            started: Tracked::new(Started::Nothing),
            title: Tracked::new(None),
            shown: Tracked::new(false),
            search: Search::new(),
        }
    }

    pub(crate) fn is_shown(&self) -> bool {
        *self.shown
    }

    pub(crate) fn is_live(&self) -> bool {
        *self.shown || self.shell().is_some()
    }

    pub(crate) fn runs(&self, serial: u64) -> bool {
        self.shell().is_some_and(|shell| shell.serial == serial)
    }

    pub(crate) fn toggle(&mut self, at: &Path) {
        match *self.shown {
            true => self.shown.set(false),
            false => self.show(at),
        }
    }

    pub(crate) fn fresh(&mut self, at: &Path) {
        self.ended();
        self.show(at);
    }

    pub(crate) fn follow(&mut self, at: &Path) {
        if self.shell().is_none_or(|shell| shell.at == at) {
            return;
        }
        self.ended();
        if *self.shown {
            self.show(at);
        }
    }

    pub(crate) fn hide(&mut self) {
        self.shown.set(false);
    }

    pub(crate) fn find(&mut self, action: Action) {
        self.search.act(action);
    }

    fn shell(&self) -> Option<&Shell> {
        match &*self.started {
            Started::Running(shell) => Some(shell),
            Started::Nothing | Started::Failed(_) => None,
        }
    }

    pub(crate) fn open_find(&mut self) {
        let selection = self.shell().and_then(|shell| shell.session.selected_text());
        self.search.open(selection.as_deref());
    }

    fn show(&mut self, at: &Path) {
        self.shown.set(true);
        if self.shell().is_some() {
            return;
        }

        let config = Config::load();
        let options = config.options().working_directory(at);
        let serial = OPENED.fetch_add(1, Ordering::Relaxed);
        let saying = self.handle.clone();
        *self.started = match Session::open(&options, move |report| {
            saying.say(move |project| project.terminal_report(serial, report));
        }) {
            Ok(session) => Started::Running(Shell {
                serial,
                at: at.to_owned(),
                session: Rc::new(session),
                font: config.font(),
            }),
            Err(error) => Started::Failed(Arc::from(error.to_string())),
        };
    }

    fn ended(&mut self) {
        *self.started = Started::Nothing;
        self.title.set(None);
    }

    pub(crate) fn report(&mut self, report: Report) {
        match report {
            Report::Title(title) => {
                self.title
                    .set(Some(title).filter(|title| !title.trim().is_empty()));
            }
            Report::ResetTitle => self.title.set(None),
            Report::Exited => {
                self.ended();
                self.shown.set(false);
            }
            Report::Ask(question) => {
                self.answer(question);
                skip_view();
            }
        }
    }

    fn answer(&self, question: Question) {
        if let Some(shell) = self.shell() {
            shell.session.answer(question);
        }
    }
}

pub(super) fn view(terminal: &Terminal) -> Element {
    column! {
        heading(terminal);
        body(terminal);
    }
    .height(TALL)
    .fill_width()
    .transition(wipe(Wipe::Down).enter(ARRIVAL))
    .fill_width()
}

fn heading(terminal: &Terminal) -> impl View {
    let title = match terminal.title.as_deref() {
        Some(title) => Label::from(title),
        None => tr!("ade.terminal.title"),
    };

    row! {
        icon(MaterialSymbol::TERMINAL)
            .extent(Length::px(GLYPH))
            .color(palette().text_muted);
        text(title).caption().muted().grow(1.0);
        button(icon(MaterialSymbol::RESTART_ALT).extent(Length::px(ACTION)))
            .icon_only()
            .ghost()
            .small()
            .on_press(act(|project| project.raise_terminal(Terminal::fresh)));
        button(icon(MaterialSymbol::CLOSE).extent(Length::px(ACTION)))
            .icon_only()
            .ghost()
            .small()
            .on_press(act(|project| project.terminal(Terminal::hide)));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::edges(0.0, 0.0, 4.0, 2.0))
    .fill_width()
}

fn body(terminal: &Terminal) -> Element {
    let shell = match &*terminal.started {
        Started::Running(shell) => shell,
        Started::Failed(error_message) => return failed(error_message).any(),
        Started::Nothing => return spacer().grow(1.0).any(),
    };

    let session = Rc::clone(&shell.session);
    let grid = screen(Rc::clone(&shell.session), shell.font.clone())
        .on_key(
            move |project: &mut Project, key: Key, modifiers: Modifiers| {
                let Some(chord) = chord(session.alternate_screen(), key, modifiers) else {
                    return false;
                };
                match chord {
                    Chord::Hide => project.terminal(Terminal::hide),
                    Chord::Fresh => project.raise_terminal(Terminal::fresh),
                    Chord::OpenFind => project.terminal(Terminal::open_find),
                }
                project.settle();
                true
            },
        )
        .search(&terminal.search)
        .grow(1.0)
        .fill_width()
        .build()
        .keyed((GRID, shell.serial));

    column! {
        find_mark(FIND);
        grid;
        find_bar(&terminal.search)
            .anchored_to(FIND)
            .on_action(given(|project, action: Action| {
                project.terminal(|terminal| terminal.find(action));
            }));
    }
    .grow(1.0)
    .fill_width()
    .any()
}

fn failed(error_message: &Arc<str>) -> impl View {
    container(notice(
        MaterialSymbol::ERROR,
        error_message.as_ref(),
        palette().danger,
    ))
    .grow(1.0)
    .fill_width()
}

fn chord(has_selection: bool, key: Key, modifiers: Modifiers) -> Option<Chord> {
    if key == Key::Escape && modifiers == Modifiers::NONE && !has_selection {
        return Some(Chord::Hide);
    }
    if !modifiers.control || modifiers.alt || modifiers.logo {
        return None;
    }
    match key.to_lowercase() {
        Key::Char('t') if modifiers.shift => Some(Chord::Fresh),
        Key::Char('t') => Some(Chord::Hide),
        Key::Char('f') if modifiers.shift => Some(Chord::OpenFind),
        _ => None,
    }
}
