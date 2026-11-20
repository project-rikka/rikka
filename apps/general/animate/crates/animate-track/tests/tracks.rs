use animate_track::{Keyframe, Sampled, Tracks};
use rikka_keyframe::Framed;
use rikka_mmd::VmdCurve;

const EASED: VmdCurve = VmdCurve {
    first: (127, 0),
    second: (127, 0),
};

#[derive(Clone, Debug, PartialEq)]
struct Key {
    id: String,
    frame: u32,
    value: f32,
    curve: VmdCurve,
}

impl Framed for Key {
    type Frame = u32;

    fn frame(&self) -> u32 {
        self.frame
    }
}

impl Keyframe for Key {
    type Id = String;

    fn id(&self) -> &String {
        &self.id
    }

    fn at_frame(self, frame: u32) -> Self {
        Self { frame, ..self }
    }
}

impl Sampled for Key {
    type Value = f32;

    fn value(&self) -> f32 {
        self.value
    }

    fn toward(&self, to: &Self, t: f32) -> f32 {
        self.value + (to.value - self.value) * to.curve.factor(t)
    }
}

fn key(id: &str, frame: u32, value: f32, curve: VmdCurve) -> Key {
    Key {
        id: id.to_owned(),
        frame,
        value,
        curve,
    }
}

#[test]
fn values_hold_outside_keys_and_follow_the_arriving_curve() {
    let mut tracks = Tracks::default();
    tracks.insert(key("a", 10, 1.0, VmdCurve::LINEAR));
    tracks.insert(key("a", 20, 3.0, VmdCurve::LINEAR));
    tracks.insert(key("a", 30, 5.0, EASED));
    let cases = [
        (0.0, 1.0),
        (10.0, 1.0),
        (15.0, 2.0),
        (20.0, 3.0),
        (40.0, 5.0),
    ];
    for (frame, value) in cases {
        let found = tracks.value_at("a", frame).expect("a keyed track");
        assert!((found - value).abs() < 1e-5, "{frame}: {found}");
    }
    let eased = tracks.value_at("a", 25.0).expect("a keyed track");
    assert!(
        eased < 4.0,
        "the curve of the key arrived at shapes the span: {eased}"
    );
    assert_eq!(tracks.value_at("b", 15.0), None);
    assert_eq!(tracks.held_at("a", 25).map(|key| key.frame), Some(20));
    assert_eq!(tracks.held_at("a", 5).map(|key| key.frame), Some(10));
}

#[test]
fn changed_from_names_the_key_before_the_first_difference() {
    let mut before = Tracks::default();
    for frame in [0, 10, 20] {
        before.insert(key("a", frame, frame as f32, VmdCurve::LINEAR));
    }
    let mut after = before.clone();
    assert_eq!(after.changed_from(&before), None);
    after.insert(key("a", 20, 7.0, VmdCurve::LINEAR));
    assert_eq!(after.changed_from(&before), Some(10));
    after.insert(key("b", 40, 1.0, VmdCurve::LINEAR));
    assert_eq!(after.changed_from(&before), Some(0));
    let mut removed = before.clone();
    removed.remove("a", 10);
    assert_eq!(removed.changed_from(&before), Some(0));
}

#[test]
fn edits_leave_copies_and_untouched_tracks_alone() {
    let mut before = Tracks::default();
    for id in ["a", "b"] {
        for frame in [0, 10] {
            before.insert(key(id, frame, frame as f32, VmdCurve::LINEAR));
        }
    }
    let mut after = before.clone();
    after.insert(key("a", 5, 9.0, VmdCurve::LINEAR));
    assert_eq!(before.keys("a").len(), 2);
    assert_eq!(after.keys("a").len(), 3);
    assert!(std::ptr::eq(before.keys("b"), after.keys("b")));
    assert_ne!(after, before);
    after.remove("a", 5);
    assert_eq!(after, before);
    assert_eq!(after.remove("a", 7), None);
}
