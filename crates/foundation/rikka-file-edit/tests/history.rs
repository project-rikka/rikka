use rikka_file_edit::{EditRequest, History};

use crate::support::Project;

#[test]
fn rewind_restores_checkpoint_state() {
    let project = Project::open();
    project.write_file("a.txt", "one\n");
    let history = History::open(project.folder.path().join(".history"));
    let recorded = project.files.with_history(&history);
    let root = project.files.root().to_owned();
    let at = |path: &str| root.join(path);

    history.set_checkpoint(10);
    recorded.write("a.txt", "two\n").expect("written");
    recorded
        .write("a.txt", "two and a half\n")
        .expect("written");
    recorded.write("made/new.txt", "fresh\n").expect("written");

    history.set_checkpoint(20);
    recorded
        .edit(&EditRequest {
            path: "a.txt",
            edits: vec![project.lines("a.txt", 1, 1, "three")],
        })
        .expect("edited");
    recorded
        .write("b.txt", "untouched later\n")
        .expect("written");
    std::fs::remove_file(at("b.txt")).expect("removed by hand");

    assert_eq!(
        history.changed_since(20).expect("read"),
        vec![at("a.txt")],
        "b.txt is already as it was when 20 began"
    );
    let rewound = history.rewind(20).expect("rewound");
    assert_eq!(rewound.restored, vec![at("a.txt")]);
    assert!(rewound.failed.is_empty());
    assert_eq!(project.read_file("a.txt"), "two and a half\n");
    assert!(history.changed_since(20).expect("read").is_empty());

    let reopened = History::open(project.folder.path().join(".history"));
    assert_eq!(
        reopened.changed_since(10).expect("read"),
        vec![at("a.txt"), at("made/new.txt")]
    );
    reopened.rewind(10).expect("rewound");
    assert_eq!(project.read_file("a.txt"), "one\n");
    assert!(!at("made/new.txt").exists());
    assert!(reopened.changed_since(0).expect("read").is_empty());
}

#[test]
fn unrecordable_change_is_not_written() {
    let project = Project::open();
    project.write_file("a.txt", "one\n");
    project.write_file("blocked", "a file where the history folder should be\n");
    let history = History::open(project.folder.path().join("blocked"));

    let refused = project.files.with_history(&history).write("a.txt", "two\n");

    assert!(refused.is_err());
    assert_eq!(project.read_file("a.txt"), "one\n");
}

#[test]
fn failed_restore_keeps_snapshot() {
    let project = Project::open();
    project.write_file("a.txt", "one\n");
    let history = History::open(project.folder.path().join(".history"));
    history.set_checkpoint(10);
    project
        .files
        .with_history(&history)
        .write("a.txt", "two\n")
        .expect("written");
    let at = project.files.root().join("a.txt");
    std::fs::remove_file(&at).expect("removed");
    std::fs::create_dir(&at).expect("a folder in its place");

    let failed = history.rewind(10).expect("the history reads");
    assert_eq!(failed.failed.len(), 1);

    std::fs::remove_dir(&at).expect("the folder is cleared");
    let retried = history.rewind(10).expect("the history reads");
    assert_eq!(retried.restored, vec![at]);
    assert_eq!(project.read_file("a.txt"), "one\n");
}
