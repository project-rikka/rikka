use animate_cast::{RESOLUTION_PRESETS, Resolution, parse_side, resolution_preset};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::{STAGE, labelled};
use crate::animate::Animate;
use crate::output::Sizing;

pub(super) const WIDTH: f32 = 380.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Purpose {
    Stage,
    Export,
}

impl Purpose {
    fn submit(self, animate: &mut Animate) {
        match self {
            Self::Stage => animate.output_mut().apply_sizing(),
            Self::Export => animate.export(),
        }
    }
}
const GLYPH: f32 = 18.0;
const SIDE_DIGITS: usize = 4;

pub(super) fn sheet(sizing: &Sizing) -> Element {
    dialog(tr!("animate.output.title"), rows(sizing, Purpose::Stage))
        .cancel(tr!("animate.output.cancel"), |animate: &mut Animate| {
            animate.output_mut().close();
        })
        .accept(tr!("animate.output.apply"), |animate: &mut Animate| {
            animate.output_mut().apply_sizing();
        })
        .ready(sizing.drafted().is_some())
        .width(WIDTH)
        .keyed(("animate-output-sheet", sizing.key()))
        .centered_on(STAGE)
        .build()
}

pub(super) fn rows(sizing: &Sizing, purpose: Purpose) -> impl View {
    column! {
        labelled(tr!("animate.output.preset"), preset_picker(sizing.shape()));
        labelled(tr!("animate.output.size"), group_sides(sizing, purpose));
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn preset_picker(shape: Resolution) -> impl View {
    let labels: Vec<Label> = RESOLUTION_PRESETS
        .iter()
        .map(|preset| {
            let size = preset.facing(shape);
            let (across, down) = preset.ratio_facing(shape);
            Label::from(format!(
                "{} × {} · {across}:{down}",
                size.width(),
                size.height()
            ))
        })
        .collect();
    combo(resolution_preset(shape))
        .options(labels)
        .placeholder(tr!("animate.output.custom"))
        .on_select(|animate: &mut Animate, chosen| animate.output_mut().choose_preset(chosen))
        .grow(1.0)
}

fn group_sides(sizing: &Sizing, purpose: Purpose) -> impl View {
    row! {
        side_field(sizing.width(), true, purpose).grow(1.0);
        text("×").muted();
        side_field(sizing.height(), false, purpose).grow(1.0);
        button(icon(MaterialSymbol::SWAP_HORIZ).extent(Length::px(GLYPH)))
            .icon_only()
            .small()
            .ghost()
            .enabled(sizing.drafted().is_none_or(|drafted| !drafted.is_square()))
            .on_press(|animate: &mut Animate| animate.output_mut().turn_sizing())
            .tooltip(tr!("animate.tip.swap_sides"));
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .grow(1.0)
}

fn side_field(typed: &TextDocument, first: bool, purpose: Purpose) -> TextField {
    let even = purpose == Purpose::Export;
    let fits = parse_side(&typed.text()).is_some_and(|side| !even || side.is_multiple_of(2));
    text_field(typed)
        .max_len(SIDE_DIGITS)
        .error(!fits)
        .autofocus(first)
        .claims_focus(first)
        .on_submit(move |animate: &mut Animate| purpose.submit(animate))
}
