use rikka_keyframe::{Around, Framed, Sequence};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Key {
    at: i64,
    value: f32,
}

impl Framed for Key {
    type Frame = i64;

    fn frame(&self) -> i64 {
        self.at
    }
}

fn key(at: i64, value: f32) -> Key {
    Key { at, value }
}

fn frames(sequence: &Sequence<Key>) -> Vec<i64> {
    sequence.keys().iter().map(|key| key.at).collect()
}

#[test]
fn insert_keeps_frames_sorted_and_replaces_the_key_on_its_frame() {
    let mut sequence: Sequence<Key> = [key(20, 2.0), key(0, 0.0), key(10, 1.0)]
        .into_iter()
        .collect();
    assert_eq!(frames(&sequence), [0, 10, 20]);
    assert_eq!(sequence.insert(key(10, 5.0)), Some(key(10, 1.0)));
    assert_eq!(sequence.insert(key(15, 3.0)), None);
    assert_eq!(frames(&sequence), [0, 10, 15, 20]);
    assert_eq!(sequence.key(10), Some(&key(10, 5.0)));
    assert_eq!(sequence.remove(15), Some(key(15, 3.0)));
    assert_eq!(sequence.remove(15), None);
    assert_eq!(sequence.last_frame(), Some(20));
}

#[test]
fn neighbours_answer_strictly_before_after_and_held() {
    let sequence: Sequence<Key> = [key(0, 0.0), key(10, 1.0), key(20, 2.0)]
        .into_iter()
        .collect();
    let at = |found: Option<&Key>| found.map(|key| key.at);
    let cases = [
        (-5, None, Some(0), Some(0)),
        (0, None, Some(10), Some(0)),
        (5, Some(0), Some(10), Some(0)),
        (10, Some(0), Some(20), Some(10)),
        (25, Some(20), None, Some(20)),
    ];
    for (frame, previous, next, held) in cases {
        assert_eq!(at(sequence.previous(frame)), previous, "previous {frame}");
        assert_eq!(at(sequence.next(frame)), next, "next {frame}");
        assert_eq!(at(sequence.held_at(frame)), held, "held {frame}");
    }
}

#[test]
fn around_holds_the_ends_and_measures_progress_between_keys() {
    let sequence: Sequence<Key> = [key(10, 1.0), key(20, 2.0), key(40, 4.0)]
        .into_iter()
        .collect();
    assert_eq!(sequence.around(0.0), Some(Around::Held(&key(10, 1.0))));
    assert_eq!(sequence.around(45.0), Some(Around::Held(&key(40, 4.0))));
    let Some(Around::Between(segment)) = sequence.around(25.0) else {
        panic!("25 lies between keys");
    };
    assert_eq!(segment.index, 1);
    assert_eq!((segment.from.at, segment.to.at), (20, 40));
    assert_eq!(segment.offset, 5.0);
    assert_eq!(segment.progress, 0.25);
    assert_eq!(Sequence::<Key>::new().around(1.0), None);
}

#[test]
fn edit_restores_order_and_keeps_the_later_of_repeated_frames() {
    let mut sequence: Sequence<Key> = [key(0, 0.0), key(10, 1.0)].into_iter().collect();
    sequence.edit(|keys| {
        keys.push(key(5, 0.5));
        keys.push(key(10, 9.0));
    });
    assert_eq!(sequence.keys(), [key(0, 0.0), key(5, 0.5), key(10, 9.0)]);
}

#[test]
fn first_change_names_the_last_frame_both_share() {
    let before: Sequence<Key> = [key(0, 0.0), key(10, 1.0), key(20, 2.0)]
        .into_iter()
        .collect();
    let mut after = before.clone();
    assert_eq!(after.first_change(&before), None);
    after.insert(key(20, 3.0));
    assert_eq!(after.first_change(&before), Some(10));
    let mut moved = before.clone();
    moved.insert(key(0, 5.0));
    assert_eq!(moved.first_change(&before), Some(0));
    assert_eq!(Sequence::new().first_change(&before), Some(0));
}
