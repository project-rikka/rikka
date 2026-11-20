use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use ade_git::{Change, Diff, Line, Patch, State};
use rikka_app::prelude::*;
use rikka_chat::{CARD_INSET, CODE_FONT_SIZE, CODE_ROW_HEIGHT, Ink, mono};
use rikka_widgets::{column, row};

use crate::hint::hint;
use crate::project::git::lines::{Read, Row};
use crate::project::git::painted::{Colours, Painted};
use crate::project::git::standing::Standing;
use crate::project::git::state::Git;
use crate::project::git::subject::Subject;
use crate::project::git::{history, pane};
use crate::project::places::Places;

const GLYPH: f32 = 14.0;

const NUMBER: f32 = 34.0;

const GUTTER: f32 = 10.0;

pub(in crate::project) fn view(git: &Git, places: &Places) -> Element {
    if git.standing() == Standing::Unversioned {
        return nothing(tr!("ade.git.unversioned"));
    }
    let Some((subject, patch)) = git.shown() else {
        return nothing(tr!("ade.git.nothing-shown"));
    };

    let body = match patch {
        None => nothing(tr!("ade.git.reading")),
        Some(Err(content)) => error_message(content),
        Some(Ok(_)) => lines(git, &places.diff, subject),
    };

    column! {
        heading(git, subject, patch.and_then(Result::ok));
        rule();
        body;
    }
    .grow(1.0)
    .fill_width()
    .any()
}

fn heading(git: &Git, subject: &Subject, patch: Option<&Patch>) -> Element {
    match subject {
        Subject::Commit(_) => match git.landed() {
            Some(commit) => history::heading(commit).any(),
            None => nothing(tr!("ade.git.reading")),
        },
        Subject::File(path) => {
            let change = git
                .status()
                .changes()
                .iter()
                .find(|change| change.path() == path);
            file(path, change, patch)
        }
    }
}

fn file(path: &Path, change: Option<&Change>, patch: Option<&Patch>) -> Element {
    let title = match change.and_then(Change::from) {
        Some(from) => format!("{} → {}", from.display(), path.display()),
        None => path.display().to_string(),
    };

    row! {
        if let Some(state) = change.map(Change::state) {
            chip(state);
        }
        mono(title, CODE_FONT_SIZE).medium().color(palette().text).grow(1.0);
        if let Some(patch) = patch {
            rikka_chat::diff_stat(patch.added(), patch.removed());
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(CARD_INSET, 10.0))
    .fill_width()
    .any()
}

fn chip(state: State) -> impl View {
    let color = pane::tint(state, palette());
    row! {
        icon(pane::glyph(state))
            .extent(Length::px(GLYPH))
            .color(color);
        text(pane::label(state)).caption().color(color).no_wrap();
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

fn lines(git: &Git, place: &Place, subject: &Subject) -> Element {
    let Some((read, painted)) = git.drawn() else {
        return nothing(tr!("ade.git.unreadable"));
    };
    let count = read.lines.rows().len();
    if count == 0 {
        return nothing(tr!("ade.git.unreadable"));
    }
    let shown = Shown {
        read: read.clone(),
        painted: Rc::clone(painted),
        colours: Colours::for_syntax(theme().syntax()),
    };

    container(
        lazy_column(count, move |slot| shown.row(slot.index))
            .estimate(CODE_ROW_HEIGHT)
            .shows(subject)
            .place(place)
            .grow(1.0)
            .fill_width(),
    )
    .padding(Inset::edges(4.0, 0.0, CARD_INSET, 0.0))
    .grow(1.0)
    .fill_width()
    .any()
}

struct Shown {
    read: Read,
    painted: Rc<Painted>,
    colours: Colours,
}

impl Shown {
    fn row(&self, index: usize) -> Element {
        let Some(row) = self.read.lines.at(index) else {
            return spacer().any();
        };
        match row {
            Row::Divider => rule().any(),
            Row::Heading { file } => match self.read.patch.files().get(file) {
                Some(diff) => titled(named(diff), diff.added(), diff.removed()),
                None => spacer().any(),
            },
            Row::Unreadable => unreadable(tr!("ade.git.binary")).any(),
            Row::Line { file, at } => {
                let Some(diff) = self.read.patch.files().get(file) else {
                    return spacer().any();
                };
                let Some(line) = diff.lines().get(at) else {
                    return spacer().any();
                };
                let spans = self.painted.spans(file, diff, self.colours);
                line_view(line, spans.get(at).cloned().unwrap_or_default())
            }
        }
    }
}

fn named(file: &Diff) -> String {
    match file.from() {
        Some(from) => format!("{} → {}", from.display(), file.path().display()),
        None => file.path().display().to_string(),
    }
}

fn titled(title: String, added: usize, removed: usize) -> Element {
    container(
        row! {
            mono(title, CODE_FONT_SIZE).medium().color(palette().text).grow(1.0);
            rikka_chat::diff_stat(added, removed);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .background(palette().surface_variant)
    .padding(Inset::xy(CARD_INSET, 6.0))
    .fill_width()
    .any()
}

fn line_view(line: &Line, spans: Arc<[TextSpan]>) -> Element {
    let muted = palette().text_muted;
    match line {
        Line::Place(content) => {
            rikka_chat::line(numbers(None, None, muted), content, Ink::Placed, spans)
        }
        Line::Added { content, at } => rikka_chat::line(
            numbers(None, Some(*at), muted),
            content,
            Ink::Arriving,
            spans,
        ),
        Line::Removed { content, at } => rikka_chat::line(
            numbers(Some(*at), None, muted),
            content,
            Ink::Leaving,
            spans,
        ),
        Line::Context { content, was, at } => rikka_chat::line(
            numbers(Some(*was), Some(*at), muted),
            content,
            Ink::Kept,
            spans,
        ),
    }
}

pub(super) fn inked(line: &Line) -> (Ink, &str) {
    match line {
        Line::Place(content) => (Ink::Placed, content.as_str()),
        Line::Added { content, .. } => (Ink::Arriving, content.as_str()),
        Line::Removed { content, .. } => (Ink::Leaving, content.as_str()),
        Line::Context { content, .. } => (Ink::Kept, content.as_str()),
    }
}

fn numbers(was: Option<u32>, at: Option<u32>, color: Color) -> Element {
    row! {
        number(was, color);
        number(at, color);
    }
    .padding(Inset::edges(0.0, GUTTER, 0.0, 0.0))
    .any()
}

fn number(at: Option<u32>, color: Color) -> impl View {
    let content = at.map(|at| at.to_string()).unwrap_or_default();
    mono(content, CODE_FONT_SIZE)
        .color(color)
        .no_wrap()
        .align(TextAlign::End)
        .width(NUMBER)
}

fn unreadable(content: Label) -> impl View {
    container(text(content).caption().muted().fill_width())
        .padding(Inset::xy(CARD_INSET, 10.0))
        .fill_width()
}

fn error_message(content: &str) -> Element {
    container(text(content).caption().color(palette().danger).fill_width())
        .padding(Inset::xy(CARD_INSET, 16.0))
        .grow(1.0)
        .fill_width()
        .any()
}

fn nothing(content: Label) -> Element {
    container(hint(content))
        .align_y(Align::Center)
        .grow(1.0)
        .fill_width()
        .any()
}
