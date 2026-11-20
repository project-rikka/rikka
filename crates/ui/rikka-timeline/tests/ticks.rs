use rikka_timeline::{Tick, Ticks};

#[test]
fn steps_land_on_seconds_and_leave_room_for_labels() {
    for rate in [24, 25, 30, 60] {
        for per_frame in [0.01, 0.2, 1.0, 4.0, 20.0, 48.0] {
            let ticks = Ticks::new(per_frame, rate, 56.0);
            assert!(
                ticks.major.is_multiple_of(ticks.minor),
                "{rate} {per_frame}"
            );
            let spacious = f64::from(ticks.major) * per_frame >= 56.0;
            let largest = ticks.major == 60 * 60 * rate;
            assert!(spacious || largest, "{rate} {per_frame}");
            if ticks.major >= rate {
                assert!(ticks.major.is_multiple_of(rate), "{rate} {per_frame}");
            }
        }
    }
}

#[test]
fn ticks_cover_the_shown_frames_from_a_whole_step() {
    let ticks = Ticks {
        major: 30,
        minor: 10,
    };
    let shown: Vec<Tick<u32>> = ticks.across(25.0, 61.5).collect();
    let frames: Vec<u32> = shown.iter().map(|tick| tick.frame).collect();
    assert_eq!(frames, [20, 30, 40, 50, 60]);
    let majors: Vec<bool> = shown.iter().map(|tick| tick.major).collect();
    assert_eq!(majors, [false, true, false, false, true]);
}
