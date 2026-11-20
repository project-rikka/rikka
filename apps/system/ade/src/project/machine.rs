use rikka_app::prelude::*;
use rikka_chat::chevron;
use rikka_computer::{Display, Status};
use rikka_ui::Feed;
use rikka_widgets::{column, row};

use crate::notice::notice;
use crate::project::act::act;
use crate::project::live::Live;

const LEAST: f32 = 150.0;

const EDGE: f32 = 8.0;

const SCREEN: &str = "ade-screen";

const GLYPH: f32 = 16.0;
const SAID: f32 = 12.0;
const CHEVRON: f32 = 18.0;

#[derive(Debug, Default)]
pub(crate) struct Machine {
    standing: Status,
    feed: Feed,
    lit: bool,
    lost: bool,
}

impl Machine {
    pub(crate) fn feed(&self) -> Feed {
        self.feed.clone()
    }

    pub(crate) fn stands(&mut self, standing: Status) {
        self.standing = standing;
        self.lost = false;
        if !standing.is_open() {
            self.lit = false;
        }
    }

    pub(crate) fn lost(&mut self) {
        self.lost = true;
    }

    pub(crate) fn is_lit(&self) -> bool {
        self.lit
    }

    pub(crate) fn light(&mut self) {
        self.lit = true;
    }
}

pub(super) fn view(machine: &Machine, folded: bool, taking: bool) -> Option<Element> {
    if !machine.standing.is_visible() {
        return None;
    }
    let display = machine.standing.display()?;

    let pane = column! {
        heading(folded);
        if !folded {
            body(machine, display, taking);
        }
    }
    .fill_width();
    let pane = match folded {
        true => pane,
        false => pane.grow(1.0),
    }
    .transition(fade() + wipe(Wipe::Up));
    Some(match folded {
        true => pane.fill_width().any(),
        false => pane.fill_width().grow(1.0).min_height(LEAST).any(),
    })
}

fn heading(folded: bool) -> impl View {
    row! {
        icon(MaterialSymbol::COMPUTER)
            .extent(Length::px(SAID))
            .color(palette().text_muted);
        text(tr!("ade.machine.screen")).caption().muted().grow(1.0);
        button(icon(chevron(!folded)).extent(Length::px(CHEVRON)))
            .icon_only()
            .ghost()
            .small()
            .on_press(act(move |project| project.fold_screen(!folded)));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::edges(4.0, 4.0, 4.0, EDGE))
    .fill_width()
}

fn body(machine: &Machine, display: Display, taking: bool) -> impl View {
    column! {
        if machine.lost {
            container(notice(
                MaterialSymbol::VISIBILITY_OFF,
                tr!("ade.machine.lost"),
                palette().danger,
            ))
            .padding(Inset::xy(EDGE, 0.0))
            .fill_width();
        }
        match machine.lit {
            true => screen(machine, display, taking && !machine.lost),
            false => waiting(),
        }
        .keyed(SCREEN);
    }
    .grow(1.0)
    .fill_width()
}

fn screen(machine: &Machine, display: Display, taking: bool) -> Element {
    container(
        Element::new(Live::new(
            machine.feed.clone(),
            display.capture_size(),
            taking,
        ))
        .fill(),
    )
    .padding(Space::SNUG)
    .grow(1.0)
    .fill_width()
    .any()
}

fn waiting() -> Element {
    container(
        column! {
            spinner().extent(Length::px(GLYPH));
            text(tr!("ade.machine.waiting")).caption().muted();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
    .center()
    .grow(1.0)
    .fill_width()
    .any()
}
