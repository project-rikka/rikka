use std::rc::Rc;

use rikka_app::prelude::*;
use rikka_thread::Summary;
use rikka_widgets::{column, row};

use crate::hint::hint;
use crate::labels;
use crate::project::act::{act, given, given_with_setup, with_setup};
use crate::project::state::Project;
use crate::project::tab::Tab;
use crate::project::{explorer, git};

const WIDTH: f32 = 268.0;
const GLYPH: f32 = 18.0;

pub(super) struct Line<'a> {
    pub(super) thread: &'a Summary,
    pub(super) place: Option<String>,
    pub(super) running: bool,
    pub(super) asking: bool,
    pub(super) wrong: bool,
}

pub(super) fn view(project: &Project) -> impl View {
    let tab = project.tab();
    let body = match tab {
        Tab::Threads => component(|project: &Project| project, threads_pane),
        Tab::Files => component(|project: &Project| project, files_pane),
        Tab::Git => component(|project: &Project| project, git_pane),
    };
    container(
        column! {
            segmented(tab.at())
                .segments(Tab::ALL.map(Tab::label))
                .glyphs(Tab::ALL.map(Tab::glyph))
                .on_select(given(|project, index: usize| {
                    project.turn_to(Tab::ALL[index.min(Tab::ALL.len() - 1)]);
                }))
                .fill_width();
            page_stack(tab.at(), body).grow(1.0).fill_width();
        }
        .gap(Space::SNUG)
        .padding(Inset::xy(10.0, 12.0))
        .fill_width()
        .fill_height(),
    )
    .width(WIDTH)
    .fill_height()
}

fn threads_pane(project: &Project) -> impl View {
    let _minute = project.minute();
    let lines = project.lines();
    column! {
        new();
        threads_list(lines.as_deref(), project.open_thread(), &project.places().threads);
    }
    .gap(Space::SNUG)
    .grow(1.0)
    .fill_width()
}

fn files_pane(project: &Project) -> Element {
    explorer::view(project.shown(), &project.places().files)
}

fn git_pane(project: &Project) -> impl View {
    let _minute = project.minute();
    git::sidebar(project.shown().git(), project.places())
}

fn new() -> impl View {
    button(
        row! {
            icon(MaterialSymbol::ADD).extent(Length::px(GLYPH));
            text(tr!("ade.sidebar.new"));
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .secondary()
    .small()
    .on_press(with_setup(|project, setup| project.begin(setup)))
    .fill_width()
}

fn threads_list(threads: Option<&[Line<'_>]>, open: &str, place: &Place) -> Element {
    let Some(threads) = threads else {
        return hint(tr!("ade.sidebar.reading")).any();
    };
    if threads.is_empty() {
        return hint(tr!("ade.sidebar.no-threads")).any();
    }
    let ids: Rc<[String]> = threads
        .iter()
        .map(|line| line.thread.id().to_owned())
        .collect();
    let selected = ids.iter().position(|id| id == open);

    list(threads.iter().map(row_for))
        .selected(selected)
        .on_select(given_with_setup(
            move |project, setup, index: usize| match ids.get(index) {
                Some(id) => project.choose(id.clone(), setup),
                None => project.begin(setup),
            },
        ))
        .place(place)
        .fill()
        .any()
}

fn row_for(line: &Line<'_>) -> ListItem {
    let title = match line.thread.title().is_empty() {
        true => tr!("ade.sidebar.untitled"),
        false => Label::from(line.thread.title()),
    };

    let deleting = line.thread.id().to_owned();
    let item = list_item(title)
        .keyed(line.thread.id())
        .trailing(labels::time_ago(line.thread.updated()))
        .on_action(
            MaterialSymbol::DELETE,
            act(move |project| project.delete(deleting.clone())),
        );
    let item = match &line.place {
        Some(place) => item.secondary(place.clone()),
        None => item,
    };

    match (line.running, line.asking, line.wrong) {
        (true, true, _) => item.icon(MaterialSymbol::HELP).tint(palette().warning),
        (true, false, _) => item.busy(true).tint(palette().accent),
        (false, _, true) => item.icon(MaterialSymbol::ERROR).tint(palette().danger),
        (false, _, false) => item.icon(MaterialSymbol::CHAT_BUBBLE),
    }
}
