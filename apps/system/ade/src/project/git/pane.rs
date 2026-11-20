use std::path::PathBuf;
use std::rc::Rc;

use ade_git::{Change, State};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::hint::hint;
use crate::labels;
use crate::notice::notice;
use crate::project::git::group::{Group, grouped};
use crate::project::git::showing::Showing;
use crate::project::git::standing::Standing;
use crate::project::git::state::Git;
use crate::project::git::subject::Subject;
use crate::project::git::{SMALL, at_git, at_git_given, head, history, primary};
use crate::project::places::Places;

const LINE: f32 = 18.0;
const SHORTEST: f32 = 56.0;
const TALLEST: f32 = 112.0;

pub(in crate::project) fn view(git: &Git, places: &Places) -> impl View {
    let standing = git.standing();
    let showing = git.showing();
    column! {
        if standing == Standing::Asking {
            hint(tr!("ade.git.reading"));
        } else if standing == Standing::Unversioned {
            hint(tr!("ade.git.unversioned"));
        } else {
            head::view(git);
            looking(showing);
            rule();
            page_stack(showing.at(), shown(git, places, showing))
                .grow(1.0)
                .fill_width();
        }
        if let Some(error_message) = git.error_message() {
            notice(MaterialSymbol::ERROR, error_message, palette().danger);
        }
    }
    .gap(Space::SNUG)
    .grow(1.0)
    .fill_width()
}

fn shown(git: &Git, places: &Places, showing: Showing) -> Element {
    let (body, place) = match showing {
        Showing::Changes => (changes(git), &places.changes),
        Showing::History => (history::commits(git).any(), &places.history),
    };
    let read = scrollable(body).place(place).grow(1.0).fill_width();
    match showing {
        Showing::Changes => column! {
            read;
            rule();
            composer(git);
        }
        .gap(Space::SNUG)
        .grow(1.0)
        .fill_width()
        .any(),
        Showing::History => read.any(),
    }
}

fn looking(showing: Showing) -> impl View {
    segmented(showing.at())
        .segments(Showing::ALL.map(Showing::label))
        .glyphs(Showing::ALL.map(Showing::glyph))
        .control_size(ControlSize::Small)
        .on_select(at_git_given(|git, index: usize| {
            git.look(Showing::ALL[index.min(Showing::ALL.len() - 1)]);
        }))
        .fill_width()
}

fn changes(git: &Git) -> Element {
    let changes = grouped(git.status().changes());
    if changes.is_empty() {
        return hint(tr!("ade.git.unchanged")).any();
    }

    let open = match git.shown() {
        Some((Subject::File(path), _)) => Some(path.as_path()),
        _ => None,
    };
    let Rows { items, paths } = rows(&changes, git);
    let paths: Rc<[Option<PathBuf>]> = paths.into();
    let at = open.and_then(|open| paths.iter().position(|path| path.as_deref() == Some(open)));

    column! {
        all(git, changes.len());
        list(items)
            .selected(at)
            .control_size(ControlSize::Small)
            .on_select(at_git_given(move |git, index: usize| {
                match paths.get(index).and_then(Option::clone) {
                    Some(path) => git.show(Subject::File(path)),
                    None => skip_view(),
                }
            }))
            .fill_width();
    }
    .gap(Space::HAIR)
    .fill_width()
    .any()
}

#[derive(Default)]
struct Rows {
    items: Vec<ListItem>,
    paths: Vec<Option<PathBuf>>,
}

impl Rows {
    fn push(&mut self, item: ListItem, path: Option<PathBuf>) {
        self.items.push(item);
        self.paths.push(path);
    }
}

fn rows(changes: &[&Change], git: &Git) -> Rows {
    let mut rows = Rows::default();
    let headed = headed(changes);
    let mut listed: Option<Group> = None;
    for change in changes {
        let group = Group::from_change(change);
        if headed && listed != Some(group) {
            rows.push(list_header(group.label()), None);
        }
        listed = Some(group);
        rows.push(row_for(change, git), Some(change.path().to_owned()));
    }
    rows
}

fn headed(changes: &[&Change]) -> bool {
    let first = changes.first().map(|change| Group::from_change(change));
    changes
        .iter()
        .any(|change| Some(Group::from_change(change)) != first)
}

fn all(git: &Git, changed: usize) -> impl View {
    let ticked = git.ticked();
    let stat = git.status().stat();
    row! {
        checkbox(ticked)
            .control_size(ControlSize::Small)
            .label(
                text(tr!("ade.git.changed", count = changed as i64))
                    .caption()
                    .medium()
                    .no_wrap(),
            )
            .on_toggle(at_git_given(move |git, _: bool| {
                git.tick_all(ticked.toggled())
            }))
            .grow(1.0);
        rikka_chat::diff_stat(stat.added(), stat.removed());
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

fn row_for(change: &Change, git: &Git) -> ListItem {
    let path = change.path();
    let ticking = path.to_owned();
    let item = list_item(labels::file_name(path))
        .keyed(path)
        .check(git.is_ticked(path))
        .on_check(at_git_given(move |git, ticked: bool| {
            git.tick(ticking.clone(), ticked)
        }))
        .icon(glyph(change.state()))
        .tint(tint(change.state(), palette()));
    match change.stat() {
        Some(stat) => rikka_chat::diff_counts(stat.added(), stat.removed())
            .fold(item, |item, (label, color)| item.tag(label, color)),
        None => item,
    }
}
pub(in crate::project) fn glyph(state: State) -> MaterialSymbol {
    match state {
        State::Added | State::Untracked => MaterialSymbol::ADD,
        State::Modified => MaterialSymbol::EDIT,
        State::Deleted => MaterialSymbol::REMOVE,
        State::Renamed => MaterialSymbol::DRIVE_FILE_MOVE,
        State::Copied => MaterialSymbol::CONTENT_COPY,
        State::Retyped => MaterialSymbol::CHANGE_CIRCLE,
        State::Conflicted => MaterialSymbol::WARNING,
    }
}

pub(super) fn tint(state: State, palette: Palette) -> Color {
    match state {
        State::Added | State::Untracked => palette.success,
        State::Deleted => palette.danger,
        State::Conflicted => palette.warning,
        State::Modified | State::Renamed | State::Copied | State::Retyped => palette.text_muted,
    }
}

pub(in crate::project) fn ink(state: State, palette: Palette) -> Color {
    match state {
        State::Added | State::Untracked | State::Copied => palette.success,
        State::Deleted => palette.danger,
        State::Conflicted => palette.danger,
        State::Modified | State::Renamed | State::Retyped => palette.warning,
    }
}

pub(in crate::project) fn label(state: State) -> Label {
    match state {
        State::Added => tr!("ade.git.state.added"),
        State::Untracked => tr!("ade.git.state.untracked"),
        State::Modified => tr!("ade.git.state.modified"),
        State::Deleted => tr!("ade.git.state.deleted"),
        State::Renamed => tr!("ade.git.state.renamed"),
        State::Copied => tr!("ade.git.state.copied"),
        State::Retyped => tr!("ade.git.state.retyped"),
        State::Conflicted => tr!("ade.git.state.conflicted"),
    }
}

fn composer(git: &Git) -> impl View {
    column! {
        text_field(git.summary())
            .placeholder(tr!("ade.git.summary"))
            .control_size(ControlSize::Small)
            .on_change(|| {})
            .on_submit(at_git(Git::commit))
            .fill_width();
        text_editor(git.description())
            .placeholder(tr!("ade.git.description"))
            .gutter(false)
            .family(Family::parse("sans-serif"))
            .font_size(SMALL)
            .line_height(LINE)
            .fits_text(SHORTEST..=TALLEST)
            .on_change(|| {});
        primary::view(git);
    }
    .gap(Space::SNUG)
    .fill_width()
}
