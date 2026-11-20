use ui_scene::{Bounce, Motion, ROW_HEIGHT, SCROLL_STEP, Scenario, WINDOW_HEIGHT};

#[test]
fn scrolling_bounces_between_the_list_ends() {
    let rows = 100;
    let furthest = rows as f32 * ROW_HEIGHT - WINDOW_HEIGHT;
    let mut bounce = Bounce::new(rows);
    let offsets: Vec<f32> = (0..200).map(|_| bounce.advance()).collect();
    assert!(
        offsets
            .windows(2)
            .all(|pair| (pair[1] - pair[0]).abs() == SCROLL_STEP)
    );
    let (lowest, highest) = offsets
        .iter()
        .fold((f32::MAX, f32::MIN), |(low, high), offset| {
            (low.min(*offset), high.max(*offset))
        });
    assert_eq!((lowest, highest), (SCROLL_STEP, furthest - SCROLL_STEP));
}

#[test]
fn scenario_names_round_trip_between_driver_and_apps() {
    for scenario in Scenario::all() {
        assert_eq!(
            scenario.to_string().parse::<Scenario>().ok(),
            Some(scenario)
        );
    }
    let cases = [
        (
            "cells-3000-one",
            Some(Scenario::Cells {
                count: 3000,
                motion: Motion::One,
            }),
        ),
        ("scroll", Some(Scenario::Scroll)),
        ("cells-100", None),
        ("cells-x-anim", None),
        ("cells-100-anim-extra", None),
        ("", None),
    ];
    for (name, expected) in cases {
        assert_eq!(name.parse::<Scenario>().ok(), expected, "{name:?}");
    }
}
