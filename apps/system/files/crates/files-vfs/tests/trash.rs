use files_core::{Location, SCHEME_TRASH};
use files_vfs::{Error, ListOptions, Task};
use rikka_file_operations::Store;

use crate::support::{Sandbox, read, vfs};

fn sandbox_with_trash(name: &str) -> Sandbox {
    let sandbox = Sandbox::new(name);
    unsafe { std::env::set_var("XDG_DATA_HOME", sandbox.dir("data")) };
    sandbox
}

#[test]
fn trash_scheme_reaches_only_trashed_items() {
    let sandbox = sandbox_with_trash("trash-outside");
    sandbox.file("kept/inside.txt", b"kept");
    let outside = Location::from_scheme_and_path(SCHEME_TRASH, sandbox.0.join("kept"));
    let vfs = vfs();

    assert!(matches!(vfs.stat(&outside), Err(Error::NotFound { .. })));
    assert!(vfs.list(&outside, ListOptions::default()).is_err());
    assert!(vfs.read(&outside.child("inside.txt")).is_err());
    let outcome = rikka_file_operations::run(
        vfs,
        Task::Delete {
            items: vec![outside],
        },
    )
    .wait();

    assert!(!outcome.failures.is_empty());
    assert_eq!(read(&sandbox.0.join("kept/inside.txt")), b"kept");
}

#[test]
fn deleting_trashed_folder_keeps_namesake() {
    let sandbox = sandbox_with_trash("trash-namesake");
    sandbox.file("project/notes.txt", b"inner");
    sandbox.file("notes.txt", b"outer");
    let vfs = vfs();
    let folder = vfs.trash(&sandbox.at("project")).expect("folder trashed");
    let namesake = vfs.trash(&sandbox.at("notes.txt")).expect("file trashed");

    assert!(matches!(vfs.delete(&folder), Err(Error::NotEmpty { .. })));
    let outcome = rikka_file_operations::run(
        std::sync::Arc::clone(&vfs),
        Task::Delete {
            items: vec![folder.clone()],
        },
    )
    .wait();

    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert!(vfs.stat(&folder).is_err());
    let listed = vfs
        .list(&Location::root_of(SCHEME_TRASH), ListOptions::default())
        .expect("the trash lists");
    assert!(listed.iter().any(|entry| entry.location == namesake));
    assert_eq!(
        vfs.restore(&namesake).expect("it comes back"),
        sandbox.at("notes.txt")
    );
}
