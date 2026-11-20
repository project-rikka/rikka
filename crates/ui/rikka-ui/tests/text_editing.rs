use rikka_ui::LineEnding;
use rikka_ui::text::{Content, Splice};

fn content_at(text: &str, at: usize) -> Content {
    let mut content = Content::new(text);
    content.place_caret(at, false);
    content
}

#[test]
fn line_endings_round_trip() {
    let content = Content::new("one\r\ntwo\r\n");
    assert_eq!(content.text(), "one\ntwo\n");
    assert_eq!(content.line_ending(), LineEnding::Crlf);
    assert_eq!(content.line_count(), 3, "caret cannot move past line break");
    assert_eq!(content.line_range(1), 4..7);

    for (file, ending) in [
        ("one\r\ntwo\n", LineEnding::Crlf),
        ("one\ntwo\r\n", LineEnding::Lf),
        ("one\rtwo", LineEnding::Cr),
        ("no break", LineEnding::Lf),
        ("", LineEnding::Lf),
    ] {
        assert_eq!(LineEnding::detect(file), ending, "{file:?}");
    }
    let read = LineEnding::normalize("one\r\ntwo\rthree");
    assert_eq!(read, "one\ntwo\nthree");
    assert_eq!(LineEnding::Crlf.join(&read), "one\r\ntwo\r\nthree");
    assert_eq!(LineEnding::Lf.join(&read), "one\ntwo\nthree");
}

#[test]
fn edits_snap_to_char_boundaries() {
    let mut content = content_at("あい", 3);
    content.insert_str("ー");
    assert_eq!(content.text(), "あーい");
    assert_eq!(content.caret(), 6);

    content.select(1, 5);
    assert_eq!(content.selection(), 0..3);
    assert!(content.delete_selection());
    assert_eq!(content.text(), "ーい");
}

#[test]
fn typing_burst_undoes_as_one_step() {
    let mut content = Content::new("");
    content.insert_str("a");
    content.insert_str("b");
    content.insert_str("c");
    assert!(content.undo());
    assert_eq!(content.text(), "");
    assert!(!content.undo());
    assert!(content.redo());
    assert_eq!(content.text(), "abc");

    content.interrupt();
    content.insert_str("de");
    assert!(content.undo());
    assert_eq!(content.text(), "abc");
}

#[test]
fn appends_keep_selection_and_undo_as_one_step() {
    let mut content = Content::new("one\n");
    content.place_caret(3, false);
    content.insert_str("!");
    content.select(0, 3);
    assert!(content.append("two\r\n"));
    assert!(content.append("three\n"));
    assert!(!content.append(""));
    assert_eq!(content.text(), "one!\ntwo\nthree\n");
    assert_eq!(content.selection(), 0..3);

    assert!(content.undo());
    assert_eq!(content.text(), "one!\n");
    assert!(content.undo());
    assert_eq!(content.text(), "one\n");
}

#[test]
fn unsaved_tracks_saved_revision() {
    let mut content = Content::new("one");
    assert!(!content.is_modified());
    content.insert_str("!");
    assert!(content.is_modified());
    content.mark_saved();
    assert!(!content.is_modified());
    assert!(content.undo());
    assert!(content.is_modified(), "undo/redo crosses save point");
}

#[test]
fn identical_set_text_is_noop() {
    let mut content = content_at("one", 1);
    content.insert_str("!");
    content.interrupt();
    let revision = content.revision();
    assert!(content.set_text("o!ne"), "caret did not reach end");
    assert!(!content.set_text("o!ne"));
    assert_eq!(content.revision(), revision);
    assert!(content.undo());
    assert_eq!(content.text(), "one", "no empty step was recorded");
}

#[test]
fn changes_notify_observers_with_splice() {
    let mut content = content_at("one two three", 4);
    content.insert_str("and ");
    let revision = content.revision();
    assert!(content.undo());
    let splices: Vec<Splice> = content
        .changes_since(revision)
        .expect("undo not reported as change")
        .collect();
    assert_eq!(splices.len(), 1);
    assert_eq!(
        (splices[0].start, splices[0].old_end, splices[0].new_end),
        (4, 8, 4)
    );

    let revision = content.revision();
    content.set_text("one six three");
    let splices: Vec<Splice> = content
        .changes_since(revision)
        .expect("set_text not reported as change")
        .collect();
    assert_eq!(splices.len(), 1);
    assert_eq!(
        (splices[0].start, splices[0].old_end, splices[0].new_end),
        (4, 7, 7)
    );

    let revision = content.revision();
    for _ in 0..1000 {
        content.interrupt();
        content.insert_str("x");
    }
    assert!(
        content.changes_since(revision).is_none(),
        "stale reader not marked for reread"
    );
}
