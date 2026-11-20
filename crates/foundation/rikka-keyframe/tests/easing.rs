use rikka_keyframe::{Ease, Easing, Family};

const CLOSE: f64 = 1e-9;

#[test]
fn every_easing_starts_at_zero_and_ends_at_one() {
    for family in Family::ALL {
        for ease in Ease::ALL {
            let easing = Easing::new(family, ease);
            assert!(easing.at(0.0).abs() < CLOSE, "{easing:?} at 0");
            assert!((easing.at(1.0) - 1.0).abs() < CLOSE, "{easing:?} at 1");
        }
    }
}

#[test]
fn penner_values_match_the_reference_formulas() {
    let cases = [
        (Family::Quad, Ease::In, 0.5, 0.25),
        (Family::Quad, Ease::Out, 0.5, 0.75),
        (Family::Cubic, Ease::InOut, 0.25, 0.0625),
        (
            Family::Sine,
            Ease::In,
            0.5,
            1.0 - (std::f64::consts::PI / 4.0).cos(),
        ),
        (Family::Expo, Ease::In, 0.5, 2f64.powf(-5.0)),
        (Family::Circ, Ease::Out, 0.5, 0.75f64.sqrt()),
        (Family::Back, Ease::In, 0.5, -0.087_697_5),
        (Family::Bounce, Ease::Out, 0.5, 0.765_625),
        (Family::Linear, Ease::InOut, 0.3, 0.3),
    ];
    for (family, ease, x, expected) in cases {
        let found = Easing::new(family, ease).at(x);
        assert!(
            (found - expected).abs() < 1e-6,
            "{family:?} {ease:?} at {x}: {found} != {expected}"
        );
    }
}

#[test]
fn out_mirrors_in_and_in_out_is_point_symmetric() {
    for family in Family::ALL {
        let into = Easing::new(family, Ease::In);
        let out = Easing::new(family, Ease::Out);
        let both = Easing::new(family, Ease::InOut);
        assert_eq!(into.reversed(), out);
        for step in 0..=20 {
            let x = f64::from(step) / 20.0;
            assert!((out.at(x) - (1.0 - into.at(1.0 - x))).abs() < CLOSE);
            assert!(
                (both.at(x) - (1.0 - both.at(1.0 - x))).abs() < CLOSE,
                "{both:?}"
            );
        }
    }
}
