use animate_edit::{CurvePreset, EasingPreset};
use rikka_keyframe::{Ease, Easing, Family};
use rikka_mmd::VmdCurve;

const FITTED: f32 = 0.04;

fn penner(family: Family, ease: Ease) -> CurvePreset {
    CurvePreset::Penner(Easing::new(family, ease))
}

fn stored() -> Vec<(CurvePreset, [u8; 4])> {
    let mut table = vec![
        (CurvePreset::Linear, [20, 20, 107, 107]),
        (CurvePreset::Smooth, [32, 13, 32, 127]),
        (CurvePreset::Basic(Ease::In), [53, 0, 127, 127]),
        (CurvePreset::Basic(Ease::Out), [0, 0, 74, 127]),
        (CurvePreset::Basic(Ease::InOut), [53, 0, 74, 127]),
    ];
    let families = [
        (Family::Sine, [45, 0, 83, 58], [46, 0, 81, 127]),
        (Family::Quad, [42, 0, 85, 42], [57, 0, 70, 127]),
        (Family::Cubic, [42, 0, 85, 0], [84, 0, 43, 127]),
        (Family::Quart, [63, 0, 95, 0], [98, 0, 29, 127]),
        (Family::Quint, [78, 0, 102, 0], [107, 0, 20, 127]),
        (Family::Expo, [85, 1, 109, 0], [114, 1, 13, 126]),
        (Family::Circ, [71, 0, 127, 58], [108, 0, 19, 127]),
    ];
    for (family, [a, b, c, d], both) in families {
        table.push((penner(family, Ease::In), [a, b, c, d]));
        table.push((
            penner(family, Ease::Out),
            [127 - c, 127 - d, 127 - a, 127 - b],
        ));
        table.push((penner(family, Ease::InOut), both));
    }
    table
}

#[test]
fn presets_keep_the_curves_saved_motions_hold() {
    let derived: Vec<(CurvePreset, [u8; 4])> = EasingPreset::all()
        .iter()
        .map(|preset| (preset.preset, preset.curve.to_bytes()))
        .collect();
    assert_eq!(derived, stored());
}

#[test]
fn penner_presets_follow_their_easing() {
    for preset in EasingPreset::all() {
        let CurvePreset::Penner(easing) = preset.preset else {
            continue;
        };
        let error = (0..=400)
            .map(|step| {
                let x = step as f32 / 400.0;
                (preset.curve.factor(x) - easing.at(f64::from(x)) as f32).abs()
            })
            .fold(0.0, f32::max);
        assert!(error <= FITTED, "{easing:?} misses by {error}");
    }
    assert_eq!(
        EasingPreset::all()[0].curve,
        VmdCurve::LINEAR,
        "linear is MMD's default curve"
    );
}
