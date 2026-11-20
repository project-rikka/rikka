use std::sync::LazyLock;

use rikka_keyframe::{CubicBezier, Ease, Easing, Family};
use rikka_mmd::VmdCurve;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CurvePreset {
    Linear,
    Smooth,
    Basic(Ease),
    Penner(Easing),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EasingPreset {
    pub preset: CurvePreset,
    pub curve: VmdCurve,
}

static FAMILIES: LazyLock<Vec<Family>> = LazyLock::new(|| {
    Family::ALL
        .into_iter()
        .filter(|family| *family != Family::Linear)
        .filter(|family| {
            Ease::ALL.into_iter().all(|ease| {
                CubicBezier::fit(Easing::new(*family, ease)).is_some_and(fits_the_square)
            })
        })
        .collect()
});

static PRESETS: LazyLock<Vec<EasingPreset>> = LazyLock::new(|| {
    let basic = [
        (CurvePreset::Linear, VmdCurve::LINEAR),
        (CurvePreset::Smooth, quantised(CubicBezier::EASE)),
        (
            CurvePreset::Basic(Ease::In),
            quantised(CubicBezier::EASE_IN),
        ),
        (
            CurvePreset::Basic(Ease::Out),
            quantised(CubicBezier::EASE_OUT),
        ),
        (
            CurvePreset::Basic(Ease::InOut),
            quantised(CubicBezier::EASE_IN_OUT),
        ),
    ];
    let penner = FAMILIES.iter().flat_map(|family| {
        Ease::ALL.into_iter().filter_map(move |ease| {
            let easing = Easing::new(*family, ease);
            Some((
                CurvePreset::Penner(easing),
                quantised(CubicBezier::fit(easing)?),
            ))
        })
    });
    basic
        .into_iter()
        .chain(penner)
        .map(|(preset, curve)| EasingPreset { preset, curve })
        .collect()
});

impl EasingPreset {
    #[must_use]
    pub fn all() -> &'static [EasingPreset] {
        &PRESETS
    }

    #[must_use]
    pub fn families() -> &'static [Family] {
        &FAMILIES
    }

    #[must_use]
    pub fn find(preset: CurvePreset) -> Option<&'static EasingPreset> {
        PRESETS.iter().find(|candidate| candidate.preset == preset)
    }
}

fn fits_the_square(bezier: CubicBezier) -> bool {
    [bezier.x1, bezier.y1, bezier.x2, bezier.y2]
        .iter()
        .all(|coordinate| (0.0..=1.0).contains(coordinate))
}

fn quantised(bezier: CubicBezier) -> VmdCurve {
    let span = f64::from(VmdCurve::SPAN);
    let byte = |share: f64| (share * span).round().clamp(0.0, span) as u8;
    VmdCurve {
        first: (byte(bezier.x1), byte(bezier.y1)),
        second: (byte(bezier.x2), byte(bezier.y2)),
    }
}
