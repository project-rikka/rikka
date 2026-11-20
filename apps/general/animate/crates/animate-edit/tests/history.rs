use animate_edit::{Change, History, Key};
use animate_motion::{MorphKey, Take};
use rikka_mmd::VmdCurve;

fn change(frame: u32) -> Change {
    Change::laying(
        &Take::default(),
        vec![Key::Morph(MorphKey {
            morph: "あ".into(),
            frame,
            weight: 1.0,
            curve: VmdCurve::LINEAR,
        })],
    )
}

fn subjects(edits: Option<Vec<(i32, Change)>>) -> Vec<i32> {
    edits
        .unwrap_or_default()
        .into_iter()
        .map(|(subject, _)| subject)
        .collect()
}

#[test]
fn new_change_clears_redo() {
    let mut history = History::default();
    history.record(1, change(0));
    history.record(2, change(5));
    assert_eq!(subjects(history.undo()), [2]);
    assert!(history.can_redo());
    history.record(1, change(9));
    assert!(!history.can_redo());
    assert_eq!(subjects(history.undo()), [1]);
    assert_eq!(subjects(history.redo()), [1]);
}

#[test]
fn grouped_changes_undo_together() {
    let mut history = History::default();
    history.record(1, change(0));
    history.record_in(Some(7), 1, change(3));
    history.record_in(Some(7), 2, change(4));
    history.record(3, change(8));
    history.record_in(Some(7), 2, change(9));
    assert_eq!(
        subjects(history.undo()),
        [2],
        "an edit in between splits them"
    );
    assert_eq!(subjects(history.undo()), [3]);
    assert_eq!(subjects(history.undo()), [1, 2]);
    assert_eq!(subjects(history.redo()), [1, 2]);
    history.forget(1);
    assert_eq!(subjects(history.undo()), [2]);
    assert!(!history.can_undo());
}

#[test]
fn gesture_merges_into_one_step_until_sealed() {
    let mut history = History::default();
    let mut merged = 0;
    for frame in [1, 2, 3] {
        history.record_gesture(4, 1, change(frame), |_, _| merged += 1);
    }
    assert_eq!(merged, 2);
    history.seal();
    history.record_gesture(4, 1, change(6), |_, _| merged += 1);
    history.record_gesture(4, 2, change(7), |_, _| merged += 1);
    assert_eq!(merged, 2, "a sealed step or another subject starts anew");
    assert_eq!(subjects(history.undo()), [2]);
    assert_eq!(subjects(history.undo()), [1]);
    assert_eq!(subjects(history.undo()), [1]);
    assert!(!history.can_undo());
}
