use rikka_ime::{Candidates, Composing, Preedit, SegmentStyle};
use rikka_theme::Corner;
use rikka_ui::{Align, Element, Space, Styled, TextColor, TextStyled, View, palette, text};
use rikka_widgets::{column, container, row, row_of, spacer};

use crate::candidates::candidates;
use crate::showing::showing;
use crate::window::Window;

pub(super) const MIN_WIDTH: f32 = 168.0;

pub(crate) fn view(window: &Window) -> Element {
    let shown = window.shown();
    match shown.is_empty() {
        true => nothing(),
        false => showing(panel(shown), !window.closing(), window.positioner()).build(),
    }
}

fn nothing() -> Element {
    spacer().width(0.0).height(0.0).grow(0.0).build()
}

fn panel(shown: &Composing) -> impl View {
    let palette = palette();
    let listing = &shown.candidates;

    let body = column! {
        if !shown.preedit.is_empty() {
            composing(&shown.preedit);
        }
        if !shown.aux.is_empty() {
            container(text(shown.aux.clone()).caption().muted()).padding(Space::TIGHT);
        }
        candidates(&listing.entries, listing.focused, listing.layout);
        footer(shown)
    }
    .gap(Space::HAIR);

    container(body)
        .background(palette.surface)
        .border(1.0, palette.outline)
        .radius(Corner::Panel)
        .glass()
        .padding(Space::TIGHT)
        .min_width(MIN_WIDTH)
        .align_self(Align::Start)
}

fn composing(preedit: &Preedit) -> impl View {
    let palette = palette();
    let runs = preedit.segments().iter().map(|segment| {
        let run = text(segment.text.clone()).body();
        match segment.style {
            SegmentStyle::Highlight => container(run.color(palette.on_accent))
                .background(palette.accent)
                .radius(Corner::Item),
            SegmentStyle::Underline => {
                container(run.color(palette.text)).border(0.0, palette.outline)
            }
        }
    });
    container(row_of(runs).align_items(Align::Center)).padding(Space::TIGHT)
}

fn footer(shown: &Composing) -> impl View {
    row! {
        text(badge(shown)).caption().color(TextColor::Accent);
        if let Some(status) = &shown.status {
            text(status.label.clone()).caption().muted();
        }
        spacer();
        text(paging(&shown.candidates)).caption().muted()
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Space::TIGHT)
}

fn badge(shown: &Composing) -> String {
    let Some(status) = &shown.status else {
        return String::new();
    };
    match status.badge() {
        Some(badge) => badge.to_owned(),
        None => status.label.chars().take(1).collect(),
    }
}

fn paging(candidates: &Candidates) -> &'static str {
    match (candidates.previous, candidates.more) {
        (true, true) => "< >",
        (true, false) => "<",
        (false, true) => ">",
        (false, false) => "",
    }
}
