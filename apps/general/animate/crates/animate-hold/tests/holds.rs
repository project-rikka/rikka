use animate_body::Side;
use animate_hold::{Hold, HoldLane, HoldTarget, Holder, Holds};
use rikka_model_core::glam::Vec3;

fn skirt() -> HoldTarget {
    HoldTarget {
        figure: None,
        at: Vec3::new(0.1, 0.7, 0.05),
        reach: 0.05,
    }
}

fn hand(side: Side, from: u32) -> Hold {
    Hold::new(Holder::Hand(side), skirt(), from)
}

fn pin(from: u32, until: Option<u32>) -> Hold {
    Hold {
        until,
        ..Hold::new(
            Holder::Pin {
                node: 3,
                at: Vec3::ZERO,
            },
            skirt(),
            from,
        )
    }
}

#[test]
fn hold_weight_spans_its_frames() {
    let hold = Hold {
        until: Some(40),
        ease: 6,
        ..hand(Side::Left, 10)
    };
    let cases = [
        (3.0, 0.0),
        (4.0, 0.0),
        (7.0, 0.5),
        (10.0, 1.0),
        (34.0, 1.0),
        (37.0, 0.5),
        (40.0, 0.0),
        (45.0, 0.0),
    ];
    for (frame, strength) in cases {
        assert!(
            (hold.strength(frame) - strength).abs() < 1.0e-5,
            "frame {frame}: {} against {strength}",
            hold.strength(frame)
        );
    }
    let sudden = Hold { ease: 0, ..hold };
    assert_eq!(sudden.strength(9.9), 0.0);
    assert_eq!(sudden.strength(10.0), 1.0);
    assert_eq!(sudden.strength(39.9), 1.0);
    assert_eq!(sudden.strength(40.0), 0.0);
}

#[test]
fn regrab_releases_previous_hold() {
    let mut holds = Holds::default();
    holds.lay(hand(Side::Left, 10));
    holds.lay(hand(Side::Right, 12));
    let later = holds.lay(hand(Side::Left, 30));
    assert_eq!(holds.len(), 3);
    let left: Vec<(u32, Option<u32>)> = holds
        .iter()
        .filter(|hold| hold.hand() == Some(Side::Left))
        .map(|hold| (hold.from, hold.until))
        .collect();
    assert_eq!(left, [(10, Some(30)), (30, None)]);
    assert_eq!(holds.get(later).map(|hold| hold.from), Some(30));
    assert_eq!(holds.holding(35).count(), 2, "the right hand still holds");

    let earlier = holds.lay(hand(Side::Left, 5));
    assert_eq!(holds.get(earlier).and_then(|hold| hold.until), Some(10));
    assert_eq!(
        holds.get(later).map(|hold| hold.from),
        Some(30),
        "laying an earlier hold leaves the ids of the others"
    );
    holds.lay(hand(Side::Left, 30));
    assert_eq!(
        holds
            .iter()
            .filter(|hold| hold.hand() == Some(Side::Left))
            .count(),
        3,
        "a grab on the same frame replaces the one there"
    );
}

#[test]
fn release_ends_or_removes_hold() {
    let mut holds = Holds::default();
    let first = holds.lay(hand(Side::Left, 10));
    holds.release(first, 25);
    assert_eq!(holds.get(first).and_then(|hold| hold.until), Some(25));
    assert!(holds.holding(24).next().is_some());
    assert!(holds.holding(25).next().is_none());
    holds.release(first, 10);
    assert!(holds.is_empty());
}

#[test]
fn retime_clamps_between_neighbour_holds() {
    let mut holds = Holds::default();
    holds.lay(Hold {
        until: Some(20),
        ..hand(Side::Left, 10)
    });
    holds.lay(hand(Side::Left, 50));
    let middle = holds.lay(Hold {
        until: Some(40),
        ..hand(Side::Left, 30)
    });
    holds.retime(middle, 5, Some(70));
    let hold = holds.get(middle).expect("the moved hold keeps its id");
    assert_eq!((hold.from, hold.until), (20, Some(50)));
    holds.retime(middle, 45, Some(30));
    assert_eq!(
        holds.get(middle).map(|hold| (hold.from, hold.until)),
        Some((20, Some(50))),
        "a backwards span is refused"
    );
}

#[test]
fn hand_closure_follows_strongest_hold() {
    let mut holds = Holds::default();
    holds.lay(Hold {
        until: Some(20),
        ..hand(Side::Right, 0)
    });
    assert_eq!(holds.closing(Side::Right, 10.0), 1.0);
    assert_eq!(holds.closing(Side::Left, 10.0), 0.0);
    assert_eq!(holds.closing(Side::Right, 30.0), 0.0);
}

#[test]
fn overlapping_pins_get_separate_lanes() {
    let mut holds = Holds::default();
    holds.lay(pin(0, Some(20)));
    holds.lay(pin(10, Some(30)));
    holds.lay(pin(25, None));
    holds.lay(hand(Side::Right, 5));
    let lanes: Vec<(HoldLane, usize)> = holds
        .lanes()
        .iter()
        .map(|lane| (lane.lane, lane.holds.len()))
        .collect();
    assert_eq!(
        lanes,
        [
            (HoldLane::Hand(Side::Right), 1),
            (HoldLane::Pin(0), 2),
            (HoldLane::Pin(1), 1)
        ]
    );
}

#[test]
fn change_reports_earliest_frame() {
    let mut before = Holds::default();
    before.lay(hand(Side::Left, 40));
    let right = before.lay(hand(Side::Right, 10));
    let mut after = before.clone();
    assert_eq!(after.changed_since(&before), None);
    let later = after.lay(hand(Side::Left, 60));
    assert_eq!(after.changed_since(&before), Some(34));
    after.remove(later);
    after.set_reach(right, 0.2);
    assert_eq!(after.changed_since(&before), Some(4));
}
