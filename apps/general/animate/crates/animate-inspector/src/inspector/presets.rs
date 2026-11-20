use animate_edit::{CurvePreset, EasingPreset};
use rikka_app::prelude::*;
use rikka_keyframe::{Ease, Easing};
use rikka_model::mmd::VmdCurve;
use rikka_timeline::{CurveTile, EasingRow, TILE_WIDTH, curve_tile, easing_grid};
use rikka_widgets::column;

use crate::Reach;

const SAMPLES: usize = 24;
const GAP: f32 = 6.0;

pub(super) fn presets(reach: Reach, shown: VmdCurve) -> impl View {
    let plain = row_of(
        [
            (CurvePreset::Linear, tr!("animate.preset.linear")),
            (CurvePreset::Smooth, tr!("animate.preset.smooth")),
        ]
        .into_iter()
        .filter_map(|(preset, caption)| {
            Some(tile(reach, EasingPreset::find(preset)?, shown).caption(caption))
        }),
    )
    .gap(GAP)
    .fill_width();
    let basic = EasingRow {
        label: tr!("animate.preset.basic"),
        cells: Ease::ALL.map(|ease| cell(reach, CurvePreset::Basic(ease), shown)),
    };
    let rows = std::iter::once(basic).chain(EasingPreset::families().iter().map(|family| {
        EasingRow::family(*family, |easing: Easing| {
            cell(reach, CurvePreset::Penner(easing), shown)
        })
    }));
    column! {
        plain;
        easing_grid(rows);
    }
    .gap(GAP)
    .fill_width()
}

fn cell(reach: Reach, preset: CurvePreset, shown: VmdCurve) -> Element {
    match EasingPreset::find(preset) {
        Some(preset) => tile(reach, preset, shown).build(),
        None => spacer().width(TILE_WIDTH).build(),
    }
}

fn tile(reach: Reach, preset: &EasingPreset, shown: VmdCurve) -> CurveTile {
    let curve = preset.curve;
    let samples = (0..=SAMPLES)
        .map(|step| curve.factor(step as f32 / SAMPLES as f32))
        .collect();
    curve_tile(samples)
        .selected(shown == curve)
        .on_press(reach.act(move |studio| studio.bend(curve)))
}
