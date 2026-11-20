use crate::support::{close, near};
use cut_project::{
    Bend, Handle, Incoming, Key, Outgoing, Preset, Scalar, Side, Smooth, Spatial, Timing, Track,
};
use rikka_keyframe::{Ease, Easing, Family};
use rikka_types::Point;

type Scalars = Track<Scalar>;

fn key(at: i64, value: f32) -> Key<Scalar> {
    Key::new(at, value)
}

fn eased(timing: Timing) -> Scalars {
    Track::new([
        key(0, 0.0).with_timing(timing),
        key(30, 90.0).with_timing(timing),
    ])
}

#[test]
fn linear_segment_interpolates_straight() {
    let track = Track::new([key(0, 0.0), key(10, 100.0)]);

    for frame in 0..10 {
        let at = f64::from(frame) + 0.25;
        assert!(near(
            track.sample(at).expect("a value"),
            10.0 * at as f32,
            1e-3
        ));
        assert!(near(track.speed(at).expect("a speed") as f32, 10.0, 1e-3));
    }
}

#[test]
fn hold_key_steps_on_next_key() {
    let track = Track::new([key(0, 1.0).with_outgoing(Outgoing::Hold), key(10, 9.0)]);

    assert_eq!(track.sample(9.99), Some(1.0));
    assert_eq!(track.sample(10.0), Some(9.0));
    assert_eq!(track.speed(5.0), Some(0.0));
}

#[test]
fn easy_ease_is_symmetric() {
    let track = eased(Timing {
        incoming: Incoming::Bezier(Handle::EASED),
        outgoing: Outgoing::Bezier(Handle::EASED),
        smooth: Smooth::Continuous,
    });

    assert!(near(track.speed(0.0).expect("a speed") as f32, 0.0, 1e-4));
    assert!(near(track.speed(30.0).expect("a speed") as f32, 0.0, 1e-4));
    assert!(near(track.sample(15.0).expect("a value"), 45.0, 1e-3));
    let early = track.sample(5.0).expect("a value");
    let late = track.sample(25.0).expect("a value");
    assert!(near(early, 90.0 - late, 1e-3));
    assert!(early < 15.0);
}

#[test]
fn bezier_speed_and_influence_define_cubic() {
    let cubed = Track::new([
        key(0, 0.0).with_outgoing(Outgoing::Bezier(Handle::new(0.0, 1.0 / 3.0))),
        key(30, 1.0).with_incoming(Incoming::Bezier(Handle::new(3.0 / 30.0, 1.0 / 3.0))),
    ]);
    let smooth = Track::new([
        key(0, 0.0).with_outgoing(Outgoing::Bezier(Handle::EASED)),
        key(30, 1.0).with_incoming(Incoming::Bezier(Handle::EASED)),
    ]);

    for step in 0..=30 {
        let progress = f64::from(step) / 30.0;
        let at = progress * 30.0;
        let cube = progress.powi(3) as f32;
        let step = (progress * progress * (3.0 - 2.0 * progress)) as f32;
        assert!(near(cubed.sample(at).expect("a value"), cube, 1e-5));
        assert!(near(smooth.sample(at).expect("a value"), step, 1e-5));
    }
}

#[test]
fn overlong_influence_is_scaled_down() {
    let track = Track::new([
        key(0, 0.0).with_outgoing(Outgoing::Bezier(Handle::new(0.0, 0.9))),
        key(10, 1.0).with_incoming(Incoming::Bezier(Handle::new(0.0, 0.6))),
    ]);

    let span = track.spans()[0];
    let (out, into) = (
        span.outgoing.expect("a handle"),
        span.incoming.expect("a handle"),
    );
    assert!(out[0] <= into[0] + 1e-9);
    assert!((out[0] - 6.0).abs() < 1e-6 && (into[0] - 6.0).abs() < 1e-6);
}

#[test]
fn presets_follow_penner_curves() {
    for curve in Family::ALL {
        for ease in Ease::ALL {
            let easing = Easing::new(curve, ease);
            let track = Track::new([
                key(0, 10.0).with_outgoing(Outgoing::Preset(Preset::whole(easing))),
                key(20, 30.0),
            ]);
            for frame in 0..=20 {
                let progress = f64::from(frame) / 20.0;
                let expected = 10.0 + 20.0 * easing.at(progress) as f32;
                assert!(near(
                    track.sample(f64::from(frame)).expect("a value"),
                    expected,
                    1e-4
                ));
            }
            assert!(close(easing.at(0.0) as f32, 0.0) && close(easing.at(1.0) as f32, 1.0));
        }
    }
    let back = Easing::new(Family::Back, Ease::Out);
    assert!((0..100).any(|step| back.at(f64::from(step) / 100.0) > 1.0));
    let bounce = Easing::new(Family::Bounce, Ease::Out);
    assert!(close(bounce.at(1.0 / 2.75) as f32, 1.0));
}

#[test]
fn auto_bezier_speed_from_neighbours() {
    let auto = |at: i64, value: f32| {
        key(at, value).with_timing(Timing {
            incoming: Incoming::Bezier(Handle::EASED),
            outgoing: Outgoing::Bezier(Handle::EASED),
            smooth: Smooth::Auto,
        })
    };
    let track = Track::new([auto(0, 0.0), auto(10, 50.0), auto(30, 50.0), auto(40, 0.0)]);

    let speeds: Vec<(f32, f32)> = track
        .keys()
        .iter()
        .map(|key| {
            let handle = |side| key.timing.handle(side).expect("a handle");
            (handle(Side::Incoming).speed, handle(Side::Outgoing).speed)
        })
        .collect();
    assert!(near(speeds[0].1, 5.0, 1e-5));
    assert!(near(speeds[1].0, 50.0 / 30.0, 1e-5));
    assert_eq!(speeds[1].0, speeds[1].1);
    assert!(near(speeds[2].0, -50.0 / 30.0, 1e-5));
    assert!(near(speeds[3].0, -5.0, 1e-5));
    let influence = track.keys()[1]
        .timing
        .handle(Side::Outgoing)
        .expect("a handle")
        .influence;
    assert!(near(influence, 1.0 / 3.0, 1e-6));
    assert!(near(
        track.speed(10.0).expect("a speed") as f32,
        50.0 / 30.0,
        1e-3
    ));
}

#[test]
fn continuous_bezier_keeps_speed() {
    let timing = Timing {
        incoming: Incoming::Bezier(Handle::new(1.0, 0.3)),
        outgoing: Outgoing::Bezier(Handle::new(1.0, 0.3)),
        smooth: Smooth::Continuous,
    };

    let dragged = timing.with_handle(Side::Outgoing, Handle::new(4.0, 0.5));

    assert_eq!(dragged.handle(Side::Incoming), Some(Handle::new(4.0, 0.3)));
    assert_eq!(dragged.handle(Side::Outgoing), Some(Handle::new(4.0, 0.5)));
    let broken = Timing {
        smooth: Smooth::Manual,
        ..timing
    }
    .with_handle(Side::Incoming, Handle::new(-2.0, 0.2));
    assert_eq!(broken.handle(Side::Outgoing), Some(Handle::new(1.0, 0.3)));
    let auto = Timing {
        smooth: Smooth::Auto,
        ..timing
    }
    .with_handle(Side::Incoming, Handle::new(2.0, 0.2));
    assert_eq!(auto.smooth, Smooth::Continuous);
    let uneven = Track::new([
        key(0, 0.0),
        key(10, 5.0).with_timing(Timing {
            incoming: Incoming::Bezier(Handle::new(1.0, 0.3)),
            outgoing: Outgoing::Bezier(Handle::new(2.0, 0.3)),
            smooth: Smooth::Continuous,
        }),
        key(20, 0.0),
    ]);
    let incoming = uneven.keys()[1].timing.handle(Side::Incoming);
    assert_eq!(incoming.map(|handle| handle.speed), Some(2.0));
}

fn spot(at: i64, x: f32, y: f32) -> Key<Spatial> {
    Key::new(at, Point::new(x, y))
}

fn travelled(track: &Track<Spatial>, from: f64, to: f64) -> f32 {
    let steps = 400;
    (0..steps)
        .map(|step| {
            let at = |step: i32| from + (to - from) * f64::from(step) / f64::from(steps);
            let (a, b) = (track.sample(at(step)), track.sample(at(step + 1)));
            a.expect("a point").distance(b.expect("a point"))
        })
        .sum()
}

#[test]
fn auto_bezier_path_is_smooth() {
    let track = Track::new([
        spot(0, 0.0, 0.0),
        spot(20, 200.0, -200.0),
        spot(40, 400.0, 0.0),
    ]);

    assert_eq!(track.sample(20.0), Some(Point::new(200.0, -200.0)));
    let bend = track.keys()[1].bend;
    assert_eq!(bend.smooth, Smooth::Auto);
    assert!(near(bend.incoming.y, 0.0, 1e-4) && near(bend.outgoing.y, 0.0, 1e-4));
    assert!(near(bend.outgoing.x, -bend.incoming.x, 1e-4) && bend.outgoing.x > 0.0);
    let path = track.path();
    assert_eq!(path.len(), 2);
    assert_eq!(path[0][0], Point::ZERO);
    assert_eq!(path[1][3], Point::new(400.0, 0.0));
    let before = track.sample(19.0).expect("a point");
    let after = track.sample(21.0).expect("a point");
    assert!(near(before.y, after.y, 1e-2));
}

#[test]
fn curved_path_uses_arc_length() {
    let curved = Bend {
        incoming: Point::new(0.0, 0.0),
        outgoing: Point::new(0.0, -300.0),
        smooth: Smooth::Manual,
        straight: false,
    };
    let landing = Bend {
        incoming: Point::new(0.0, -300.0),
        ..curved
    };
    let track = Track::new([
        spot(0, 0.0, 0.0).with_bend(curved),
        spot(60, 300.0, 0.0).with_bend(landing),
    ]);

    let first = travelled(&track, 0.0, 20.0);
    let second = travelled(&track, 20.0, 40.0);
    let third = travelled(&track, 40.0, 60.0);
    assert!(near(first, second, first * 1e-3) && near(second, third, second * 1e-3));
    let speed = track.speed(30.0).expect("a speed") as f32;
    assert!(near(
        speed * 60.0,
        first + second + third,
        (first + second + third) * 1e-3
    ));
}
