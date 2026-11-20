mod composer;
mod conversation;
mod header;
mod servers;
mod threads;

use rikka_chat::Transcript;
use rikka_theme::Corner;
use rikka_types::{Key, Modifiers};
use rikka_ui::search::Action;
use rikka_ui::{
    Align, DurationToken, Edge, Inset, Motion, Property, Styled, Transition, View, is_exiting,
    offset, palette, slide,
};
use rikka_widgets::{column, container, find_bar, find_mark, page_stack};
use shell_kit::sheet;

use crate::panel::{Panel, Tab};

pub(crate) const WIDTH: f32 = 420.0;
pub(crate) const MARGIN: f32 = 8.0;

pub(crate) const EXIT_DURATION: DurationToken = DurationToken::Short4;

const BORDER: f32 = 1.0;

const FIND: &str = "assistant-find";

pub(crate) fn view(panel: &Panel) -> impl View {
    column! {
        if !is_exiting() {
            framed(panel)
                .transition(past_edge())
                .fill_width()
                .grow(1.0);
        }
    }
    .padding(Inset::edges(0.0, 0.0, 0.0, MARGIN))
    .fill()
}

fn past_edge() -> Transition {
    (slide(Edge::Leading) + offset(-MARGIN, 0.0))
        .exit(Motion::exit(EXIT_DURATION).curve(Property::Spatial))
}

fn framed(panel: &Panel) -> impl View {
    let palette = palette();
    let (at, body) = match panel.tab() {
        Tab::Conversation => (0, conversation::view(panel)),
        Tab::Threads => (1, threads::view(panel)),
        Tab::Servers => (2, servers::view(panel)),
    };
    let composing = panel.tab() != Tab::Servers;

    let pane = column! {
        header::view(panel);
        find_mark(FIND);
        page_stack(at, body).grow(1.0).fill_width();
        if composing {
            composer::view(panel);
        }
        find_bar(panel.transcript().finding().search())
            .anchored_to(FIND)
            .on_action(|transcript: &mut Transcript, action: Action| transcript.find(action))
    }
    .align_items(Align::Start)
    .fill_width()
    .grow(1.0);

    container(sheet(pane, on_key).keeps_tab())
        .background(palette.surface)
        .border(BORDER, palette.outline)
        .radius(Corner::Window)
        .glass()
        .fill_width()
        .grow(1.0)
}

fn on_key(panel: &mut Panel, key: Key, modifiers: Modifiers) -> bool {
    match key {
        Key::Escape if panel.transcript().finding().is_open() => panel.find(Action::Close),
        Key::Escape if panel.tab() != Tab::Conversation => panel.show(Tab::Conversation),
        Key::Escape => panel.close(),
        Key::Enter if modifiers.control && panel.tab() == Tab::Conversation => panel.send(),
        Key::Char('f') if modifiers == Modifiers::CONTROL => panel.open_find(),
        _ => return false,
    }
    true
}
