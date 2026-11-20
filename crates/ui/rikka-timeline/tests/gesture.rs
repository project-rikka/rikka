use rikka_timeline::{KeyShift, Pick, TimeScale, ZoomRange, press};

#[test]
fn pressing_keys_follows_the_selection_rules() {
    let picked = [1, 2, 3];
    let cases = [
        (vec![2], true, Pick::Toggled(vec![1, 3])),
        (
            vec![4],
            true,
            Pick::Held {
                picked: vec![1, 2, 3, 4],
                solo: None,
            },
        ),
        (
            vec![2],
            false,
            Pick::Held {
                picked: vec![1, 2, 3],
                solo: Some(vec![2]),
            },
        ),
        (
            vec![4, 5],
            false,
            Pick::Held {
                picked: vec![4, 5],
                solo: None,
            },
        ),
    ];
    for (hit, adding, expected) in cases {
        assert_eq!(
            press(&picked, hit.clone(), adding),
            expected,
            "{hit:?} {adding}"
        );
    }
}

#[test]
fn shifting_keys_counts_whole_frames_within_reach_and_snaps() {
    let mut scale = TimeScale::new(
        10.0,
        ZoomRange {
            farthest: 0.1,
            nearest: 48.0,
        },
    );
    scale.set_width(1_000.0);
    scale.set_end(1_000.0);
    let mut shift = KeyShift::new(100.0, 40u32).within(-40..=20);
    assert!(shift.drag(134.0, &scale, |_| None));
    assert_eq!(shift.by(), 3);
    assert!(!shift.drag(132.0, &scale, |_| None));
    shift.drag(1_000.0, &scale, |_| None);
    assert_eq!(shift.by(), 20);
    shift.drag(-1_000.0, &scale, |_| None);
    assert_eq!(shift.by(), -40);
    shift.drag(160.0, &scale, |at| (at == 46).then_some(4));
    assert_eq!((shift.by(), shift.snapped()), (10, Some(50)));
    shift.drag(100.0 + 10.0 * 19.0, &scale, |_| Some(5));
    assert_eq!((shift.by(), shift.snapped()), (19, None));
}
