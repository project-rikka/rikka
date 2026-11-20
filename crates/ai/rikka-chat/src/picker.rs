use std::sync::Arc;

use crate::chat::Chat;
use crate::offer::{Look, Offer};
use crate::offering::Offering;
use crate::prelude::*;
use rikka_widgets::{column, row};

const ROW: f32 = 30.0;
const SHOWN: f32 = ROW * 8.0;
const GLYPH: f32 = 16.0;
const GAP: f32 = 10.0;
const RISE: f32 = 10.0;

const OFFERS: &str = "chat-offers";

const LETTERS: usize = 76;

pub(crate) fn view<C: Chat>(offering: &Offering) -> Element {
    let offers = Arc::clone(offering.offers());
    let selected = offering.selected();
    let seats = lazy_column(offers.len(), move |slot| {
        seat::<C>(slot.index, &offers[slot.index], slot.index == selected)
    })
    .uniform(ROW)
    .lit(true)
    .selected(selected)
    .on_activate(C::take())
    .activates_on_press(true)
    .focusable(false)
    .padding(Space::TIGHT)
    .max_height(SHOWN)
    .fill_width();

    let arriving = column! {
        seats;
        rule();
    }
    .fill_width()
    .transition((fade() + offset(0.0, RISE)).enter(Curve::Spring(Spring::DEFAULT_SPATIAL)));

    container(arriving).fill_width().build().keyed(OFFERS)
}

fn seat<C: Chat>(at: usize, offer: &Offer, selected: bool) -> Container {
    let label = match offer.look() {
        Look::Path { under, leaf } => path(under, leaf, selected),
        Look::Named { name, hint, about } => named(name, hint.clone(), about.clone(), selected),
    };
    let point = C::point();

    container(
        row! {
            icon(offer.symbol())
                .extent(Length::px(GLYPH))
                .color(quiet(selected));
            label.align_items(Align::Center).grow(1.0);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(GAP, 0.0))
    .height(ROW)
    .align_y(Align::Center)
    .fill_width()
    .on_hover(Handler::relaying(
        move |reach, (over,): (bool,)| match over {
            true => point.call(reach, (at,)),
            false => {
                skip_view();
                Some(())
            }
        },
    ))
}

fn quiet(selected: bool) -> Color {
    match selected {
        true => palette().on_selection,
        false => palette().text_muted,
    }
}

fn path(under: &str, leaf: &str, selected: bool) -> Flex {
    row! {
        if !under.is_empty() {
            text(shortened(under, leaf))
                .label()
                .color(quiet(selected))
                .no_wrap();
        }
        text(leaf.to_owned())
            .label()
            .medium()
            .color(palette().ink(selected))
            .no_wrap();
    }
}

fn named(name: &str, hint: Option<Label>, about: Label, selected: bool) -> Flex {
    row! {
        text(name.to_owned())
            .label()
            .medium()
            .color(palette().ink(selected))
            .no_wrap();
        if let Some(hint) = hint {
            text(hint).label().color(quiet(selected)).no_wrap();
        }
        text(about)
            .label()
            .color(quiet(selected))
            .no_wrap()
            .grow(1.0);
    }
    .gap(Space::SNUG)
}

fn shortened(under: &str, leaf: &str) -> String {
    let room = LETTERS.saturating_sub(leaf.chars().count());
    let folders = under.chars().count();
    if folders <= room {
        return under.to_owned();
    }
    let from = under
        .char_indices()
        .nth(folders - room.saturating_sub(1))
        .map_or(under.len(), |(at, _)| at);
    format!("…{}", &under[from..])
}
