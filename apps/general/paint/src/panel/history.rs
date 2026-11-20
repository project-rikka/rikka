use std::sync::Arc;

use paint_document::Action;
use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::action;
use crate::paint::Paint;
use crate::tab::Tab;

const ROW: f32 = 36.0;
const GLYPH: f32 = 18.0;
const CURRENT_TINT: f32 = 0.16;
const UNDONE: f32 = 0.45;

pub(crate) fn list(tab: &Tab) -> impl View {
    let history_view = tab.history_view();
    let actions = Arc::clone(&history_view.actions);
    let current = history_view.current;
    let saved = history_view.saved;
    lazy_column(actions.len(), move |slot| {
        entry(
            &actions[slot.index],
            slot.index,
            current,
            saved == Some(slot.index),
        )
        .build()
    })
    .uniform(ROW)
    .reveals(Some(current))
    .padding(Space::TIGHT)
    .fill()
}

fn entry(action: &Action, index: usize, current: usize, saved: bool) -> impl View {
    let (fill, ink) = match index == current {
        true => (palette().accent.scale_alpha(CURRENT_TINT), palette().accent),
        false => (Color::TRANSPARENT, palette().text_muted),
    };
    let face = row! {
        icon(action::symbol(action)).extent(Length::px(GLYPH)).color(ink);
        text(action::name(action)).no_wrap().shrink(1.0);
        spacer().grow(1.0);
        if saved {
            icon(MaterialSymbol::SAVE)
                .extent(Length::px(GLYPH))
                .color(palette().text_muted);
        }
    }
    .gap(Space::SNUG)
    .centered()
    .fill_width();
    let face = container(face)
        .padding(Inset::xy(Space::SNUG, Space::TIGHT))
        .background(fill)
        .radius(Corner::Item)
        .on_press(move |paint: &mut Paint| paint.revisit(index))
        .focusless()
        .height(ROW)
        .fill_width();
    match index > current {
        true => face.opacity(UNDONE).any(),
        false => face.any(),
    }
}
