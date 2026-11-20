use rikka_keyframe::{CubicBezier, Ease, Easing, Family};

const STEPS: u32 = 400;

fn largest_error(bezier: CubicBezier, easing: Easing) -> f64 {
    (0..=STEPS)
        .map(|step| {
            let x = f64::from(step) / f64::from(STEPS);
            (bezier.at(x) - easing.at(x)).abs()
        })
        .fold(0.0, f64::max)
}

#[test]
fn css_keywords_evaluate_like_browsers() {
    let cases = [
        (CubicBezier::EASE, 0.5, 0.802_403_4),
        (CubicBezier::EASE_IN, 0.5, 0.315_356_9),
        (CubicBezier::EASE_OUT, 0.5, 0.684_643_1),
        (CubicBezier::EASE_IN_OUT, 0.5, 0.5),
        (CubicBezier::LINEAR, 0.37, 0.37),
    ];
    for (bezier, x, expected) in cases {
        let found = bezier.at(x);
        assert!(
            (found - expected).abs() < 1e-6,
            "{bezier:?} at {x}: {found} != {expected}"
        );
    }
}

#[test]
fn fits_stay_within_their_bound() {
    let bound = |ease: Ease| match ease {
        Ease::In | Ease::Out => 0.015,
        Ease::InOut => 0.045,
    };
    for family in Family::ALL {
        for ease in Ease::ALL {
            let easing = Easing::new(family, ease);
            match (family, CubicBezier::fit(easing)) {
                (Family::Elastic | Family::Bounce, fitted) => {
                    assert_eq!(fitted, None, "{easing:?} has no single cubic");
                }
                (_, None) => panic!("{easing:?} has a fit"),
                (_, Some(bezier)) => {
                    let error = largest_error(bezier, easing);
                    assert!(error <= bound(ease), "{easing:?} misses by {error}");
                    let square = [bezier.x1, bezier.y1, bezier.x2, bezier.y2]
                        .iter()
                        .all(|coordinate| (0.0..=1.0).contains(coordinate));
                    assert!(
                        square || family.overshoots(),
                        "{easing:?} leaves the square"
                    );
                }
            }
        }
    }
}

#[test]
fn fits_keep_the_easing_end_tangents() {
    for family in [
        Family::Sine,
        Family::Quad,
        Family::Cubic,
        Family::Quart,
        Family::Quint,
        Family::Circ,
    ] {
        let into = CubicBezier::fit(Easing::new(family, Ease::In)).expect("fits");
        let out = CubicBezier::fit(Easing::new(family, Ease::Out)).expect("fits");
        let both = CubicBezier::fit(Easing::new(family, Ease::InOut)).expect("fits");
        assert_eq!(into.y1, 0.0, "{family:?} In leaves at rest");
        assert_eq!(out, into.reflected(), "{family:?} Out mirrors In");
        assert_eq!(
            (both.y1, both.y2),
            (0.0, 1.0),
            "{family:?} InOut rests at both ends"
        );
        assert!(
            (both.x1 + both.x2 - 1.0).abs() < 1e-12,
            "{family:?} InOut is symmetric"
        );
    }
    let circle = CubicBezier::fit(Easing::new(Family::Circ, Ease::In)).expect("fits");
    assert_eq!(circle.x2, 1.0, "Circ In arrives vertically");
}

#[test]
fn a_quadratic_fits_almost_exactly() {
    let easing = Easing::new(Family::Quad, Ease::In);
    let bezier = CubicBezier::fit(easing).expect("fits");
    assert!(largest_error(bezier, easing) < 1e-3);
}
