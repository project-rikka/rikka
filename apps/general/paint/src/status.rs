use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::camera::{self, Camera};
use crate::command::Command;
use crate::ink::Ink;
use crate::paint::Paint;
use crate::pointer::Pointer;
use crate::selection::Selection;
use crate::tab::Tab;
use crate::wording;

const GLYPH: f32 = 16.0;
const SPINNER: f32 = 14.0;
const READOUT: f32 = 96.0;
const SELECTED: f32 = 128.0;
const MEASURED: f32 = 136.0;
const PERCENT: f32 = 56.0;
const SLIDER: f32 = 140.0;

pub(crate) fn bar(tab: &Tab) -> impl View {
    let facts = tab.facts();
    let loaded = tab.is_loaded();
    row! {
        if let Some(facts) = facts {
            fact(
                MaterialSymbol::PHOTO_SIZE_SELECT_LARGE,
                tr!("paint.status.size", width = facts.width, height = facts.height),
            );
            pointer(tab.pointer().clone());
            measured(tab.ink().clone());
            selected(tab.selection().clone());
            text(facts.format.name()).caption().muted().no_wrap();
            text(wording::profile(facts.profile.as_ref()))
                .caption()
                .muted()
                .no_wrap()
                .shrink(1.0);
        }
        if tab.is_saving() {
            row! {
                spinner().extent(Length::px(SPINNER));
                text(tr!("paint.status.saving"))
                .caption()
                .muted()
                .no_wrap();
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center);
        }
        if let Some(error) = tab.error_message() {
            row! {
                icon(MaterialSymbol::BROKEN_IMAGE)
                    .extent(Length::px(GLYPH))
                    .color(palette().danger);
                text(error.clone()).caption().no_wrap().shrink(1.0);
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .shrink(1.0);
        }
        spacer().grow(1.0);
        if loaded {
            zoom(tab.camera().clone());
        }
    }
    .gap(Space::ROOMY)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
    .fill_width()
}

fn fact(symbol: MaterialSymbol, value: Label) -> impl View {
    row! {
        icon(symbol).extent(Length::px(GLYPH)).color(palette().text_muted);
        text(value).caption().no_wrap();
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

fn pointer(pointer: Pointer) -> impl View {
    row! {
        icon(MaterialSymbol::NEAR_ME)
            .extent(Length::px(GLYPH))
            .color(palette().text_muted);
        live_observed(move || {
            let at = pointer.at();
            let shown = at.map_or_else(String::new, |(x, y)| format!("{x}, {y}"));
            text(shown).caption().no_wrap()
        })
        .width(READOUT);
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

fn measured(ink: Ink) -> impl View {
    live_observed(move || {
        let measurement = ink.measure().map(wording::measure);
        row! {
            if measurement.is_some() {
                icon(MaterialSymbol::STRAIGHTEN)
                    .extent(Length::px(GLYPH))
                    .color(palette().text_muted);
            }
            text(measurement.unwrap_or_default()).caption().no_wrap();
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
    })
    .width(MEASURED)
}

fn selected(selection: Selection) -> impl View {
    row! {
        icon(MaterialSymbol::HIGHLIGHT_ALT)
            .extent(Length::px(GLYPH))
            .color(palette().text_muted);
        live_observed(move || {
            let shown = selection.shown_bounds().map_or_else(
                || Label::from(String::new()),
                |area| tr!("paint.status.size", width = area.width, height = area.height),
            );
            text(shown).caption().no_wrap()
        })
        .width(SELECTED);
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

fn zoom(camera: Camera) -> impl View {
    let percent = camera.clone();
    let sliding = camera.clone();
    row! {
        button(icon(MaterialSymbol::ZOOM_OUT))
            .ghost()
            .small()
            .icon_only()
            .focusless()
            .on_press(|paint: &mut Paint| paint.zoom(false))
            .tooltip(Command::ZoomOut.tooltip(tr!("paint.command.zoom_out")));
        live_observed(move || {
            slider(sliding.scale().ln())
                .range(camera::LEAST.ln()..=camera::MOST.ln())
                .control_size(ControlSize::Small)
                .on_change(|paint: &mut Paint, value: f32| paint.zoom_to(value.exp()))
        })
        .width(SLIDER);
        button(icon(MaterialSymbol::ZOOM_IN))
            .ghost()
            .small()
            .icon_only()
            .focusless()
            .on_press(|paint: &mut Paint| paint.zoom(true))
            .tooltip(Command::ZoomIn.tooltip(tr!("paint.command.zoom_in")));
        live_observed(move || {
            text(wording::percent(percent.scale()))
                .caption()
                .no_wrap()
                .align(TextAlign::End)
        })
        .width(PERCENT);
        button(text(tr!("paint.command.fit")).caption())
            .ghost()
            .small()
            .focusless()
            .on_press(|paint: &mut Paint| paint.fit());
        button(text("100%").caption())
            .ghost()
            .small()
            .focusless()
            .on_press(|paint: &mut Paint| paint.actual_size());
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}
