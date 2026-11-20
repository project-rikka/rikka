use rikka_app::prelude::*;
use rikka_image_edit::{Intent, Resampling};
use rikka_psd::BlendMode;
use rikka_widgets::{NumberField, column, row};

use crate::paint::Paint;
use crate::wording;

pub(crate) use rikka_effects::{Scale, Unit};
use rikka_effects_forms::ScaleControls;

const FIELD: f32 = 96.0;
const PERCENT_FIELD: f32 = 76.0;

pub(crate) fn pixels<M>(value: u32, on_change: impl IntoHandler<(f32,), (), M>) -> impl View {
    number_field(value as f32)
        .range(1.0..=MAX_PIXELS)
        .unit(tr!("paint.unit.pixels"))
        .on_commit(on_change)
        .on_submit(Paint::apply_session)
        .width(FIELD)
}

pub(crate) const MAX_PIXELS: f32 = 300_000.0;

pub(crate) fn percent<M>(value: f32, on_change: impl IntoHandler<(f32,), (), M>) -> impl View {
    number_field(value)
        .range(0.01..=10_000.0)
        .decimals(2)
        .unit("%")
        .on_commit(on_change)
        .on_submit(Paint::apply_session)
        .width(PERCENT_FIELD)
}

pub(crate) fn amount(
    label: Label,
    value: f32,
    scale: &Scale,
    on_change: impl Fn(&mut Paint, f32) + Clone + 'static,
) -> Element {
    column! {
        text(label).caption().muted();
        sliding(value, scale, on_change);
    }
    .gap(Space::TIGHT)
    .fill_width()
    .any()
}

pub(crate) fn sliding(
    value: f32,
    scale: &Scale,
    on_change: impl Fn(&mut Paint, f32) + Clone + 'static,
) -> Element {
    row! {
        scale
            .slider(value)
            .on_change(on_change.clone())
            .grow(1.0);
        field(value, scale, on_change);
    }
    .centered()
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

pub(crate) fn field(
    value: f32,
    scale: &Scale,
    on_change: impl Fn(&mut Paint, f32) + Clone + 'static,
) -> NumberField {
    scale
        .field(value)
        .on_commit(on_change)
        .on_submit(Paint::apply_session)
        .width(PERCENT_FIELD)
}

pub(crate) fn toggle<M>(
    label: Label,
    on: bool,
    on_toggle: impl IntoHandler<(bool,), (), M>,
) -> Element {
    checkbox(on).label(text(label)).on_toggle(on_toggle).any()
}

pub(crate) fn resampling<M>(
    selected: Resampling,
    on_select: impl IntoHandler<(usize,), (), M>,
) -> impl View {
    let at = Resampling::ALL.iter().position(|kind| *kind == selected);
    combo(at)
        .options(Resampling::ALL.map(wording::resampling))
        .on_select(on_select)
        .fill_width()
}

pub(crate) fn intent<M>(
    selected: Intent,
    on_select: impl IntoHandler<(usize,), (), M>,
) -> impl View {
    let at = Intent::ALL.iter().position(|intent| *intent == selected);
    combo(at)
        .options(Intent::ALL.map(wording::intent))
        .on_select(on_select)
        .fill_width()
}

pub(crate) fn swatch<M>(color: Color, on_change: impl IntoHandler<(Color,), (), M>) -> impl View {
    row! {
        color_well(color).on_change(on_change);
        text(wording::color(color)).caption().muted().no_wrap();
    }
    .centered()
    .gap(Space::SNUG)
}

pub(crate) fn blends(group: bool) -> &'static [BlendMode] {
    match group {
        true => &BlendMode::ALL,
        false => &BlendMode::ALL[1..],
    }
}

pub(crate) fn blend<M>(
    selected: BlendMode,
    group: bool,
    on_select: impl IntoHandler<(usize,), (), M>,
) -> impl View {
    let modes = blends(group);
    combo(modes.iter().position(|mode| *mode == selected))
        .options(modes.iter().map(|mode| wording::blend(*mode)))
        .on_select(on_select)
        .fill_width()
}
