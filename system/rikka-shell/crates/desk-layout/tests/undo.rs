use desk_layout::Undo;

#[test]
fn undo_redo_walk_history() {
    let mut undo = Undo::new();
    undo.record("a");
    undo.record("b");
    assert_eq!(undo.earlier(), Some(&"b"));
    assert_eq!(undo.later(), None);

    assert_eq!(undo.undo("c"), Some("b"));
    assert_eq!(undo.earlier(), Some(&"a"));
    assert_eq!(undo.later(), Some(&"c"));

    assert_eq!(undo.redo("b"), Some("c"));
    assert_eq!(undo.later(), None);
    assert_eq!(undo.earlier(), Some(&"b"));

    undo.undo("c");
    undo.record("d");
    assert!(!undo.can_redo(), "a new edit drops what could be redone");
    assert_eq!(undo.earlier(), Some(&"d"));
}
