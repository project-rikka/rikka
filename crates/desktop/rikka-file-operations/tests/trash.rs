use rikka_file_operations::{Placement, Task};

use crate::support::{Sandbox, finish, read};

#[test]
fn trashed_items_come_back_where_they_were() {
    let sandbox = Sandbox::with_trash();
    sandbox.file("project/notes.txt", b"inner");
    sandbox.file("notes.txt", b"outer");

    let trashed = finish(Task::Trash {
        items: vec![sandbox.at("project"), sandbox.at("notes.txt")],
    });

    assert!(trashed.failures.is_empty(), "{:?}", trashed.failures);
    assert!(sandbox.names("").is_empty());
    let restored = finish(Task::Restore {
        items: trashed
            .placed
            .iter()
            .map(|placement| placement.to.clone())
            .collect(),
    });

    assert!(restored.failures.is_empty(), "{:?}", restored.failures);
    assert_eq!(
        restored.placed,
        trashed
            .placed
            .iter()
            .map(Placement::reversed)
            .collect::<Vec<_>>()
    );
    assert_eq!(read(&sandbox.at("project/notes.txt")), b"inner");
    assert_eq!(read(&sandbox.at("notes.txt")), b"outer");
}
