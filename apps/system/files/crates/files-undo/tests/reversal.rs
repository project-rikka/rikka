use files_undo::{Direction, History, Kind, Step};
use files_vfs::Task;
use rikka_file_operations::Store;

use crate::support::{Sandbox, read};

#[test]
fn undo_move_returns_items_and_redo_moves_again() {
    let sandbox = Sandbox::new("move");
    sandbox.file("src/a.txt", b"a");
    sandbox.file("src/shared/new.txt", b"new");
    sandbox.file("dst/shared/old.txt", b"old");
    let mut history = History::default();

    sandbox.perform(
        &mut history,
        Task::Move {
            sources: vec![sandbox.at("src/a.txt"), sandbox.at("src/shared")],
            into: sandbox.at("dst"),
        },
    );
    assert_eq!(history.next(Direction::Undo), Some(Kind::Move));

    sandbox.reverse(&mut history, Direction::Undo);
    assert_eq!(read(&sandbox.path("src/a.txt")), b"a");
    assert_eq!(read(&sandbox.path("src/shared/new.txt")), b"new");
    assert_eq!(read(&sandbox.path("dst/shared/old.txt")), b"old");
    assert!(!sandbox.path("dst/a.txt").exists());
    assert!(!sandbox.path("dst/shared/new.txt").exists());
    assert_eq!(history.next(Direction::Redo), Some(Kind::Move));

    sandbox.reverse(&mut history, Direction::Redo);
    assert_eq!(read(&sandbox.path("dst/a.txt")), b"a");
    assert_eq!(read(&sandbox.path("dst/shared/new.txt")), b"new");
    assert!(!sandbox.path("src/a.txt").exists());
    assert_eq!(history.next(Direction::Undo), Some(Kind::Move));
}

#[test]
fn undo_copy_trashes_only_copies_recoverably() {
    let sandbox = Sandbox::new("copy");
    sandbox.file("src/shared/new/inner.txt", b"new");
    sandbox.file("dst/shared/old.txt", b"old");
    let mut history = History::default();

    sandbox.perform(
        &mut history,
        Task::Copy {
            sources: vec![sandbox.at("src/shared")],
            into: sandbox.at("dst"),
        },
    );
    sandbox.reverse(&mut history, Direction::Undo);
    assert!(!sandbox.path("dst/shared/new").exists());
    assert_eq!(read(&sandbox.path("dst/shared/old.txt")), b"old");
    assert_eq!(read(&sandbox.path("src/shared/new/inner.txt")), b"new");

    sandbox.reverse(&mut history, Direction::Redo);
    assert_eq!(read(&sandbox.path("dst/shared/new/inner.txt")), b"new");
    assert_eq!(history.next(Direction::Undo), Some(Kind::Copy));
}

#[test]
fn undo_trash_restores_and_redo_trashes_again() {
    let sandbox = Sandbox::new("trash");
    sandbox.file("notes/plan.txt", b"plan");
    sandbox.file("notes/deep/idea.txt", b"idea");
    let mut history = History::default();

    sandbox.perform(
        &mut history,
        Task::Trash {
            items: vec![sandbox.at("notes/plan.txt"), sandbox.at("notes/deep")],
        },
    );
    assert!(!sandbox.path("notes/plan.txt").exists());
    assert!(!sandbox.path("notes/deep").exists());

    sandbox.reverse(&mut history, Direction::Undo);
    assert_eq!(read(&sandbox.path("notes/plan.txt")), b"plan");
    assert_eq!(read(&sandbox.path("notes/deep/idea.txt")), b"idea");

    sandbox.reverse(&mut history, Direction::Redo);
    assert!(!sandbox.path("notes/plan.txt").exists());
    assert!(!sandbox.path("notes/deep").exists());

    sandbox.reverse(&mut history, Direction::Undo);
    assert_eq!(read(&sandbox.path("notes/plan.txt")), b"plan");
    assert_eq!(read(&sandbox.path("notes/deep/idea.txt")), b"idea");
}

#[test]
fn rename_and_creation_reverse_in_order() {
    let sandbox = Sandbox::new("create-rename");
    let created = sandbox.at("New folder");
    sandbox
        .vfs()
        .create_directory(&created)
        .expect("create folder");
    let mut history = History::default();
    history.record(Step::new_folder(created.clone()));
    let renamed = sandbox
        .vfs()
        .rename(&created, "Plans")
        .expect("rename folder");
    history.record(Step::renamed(created, renamed));

    assert_eq!(history.next(Direction::Undo), Some(Kind::Rename));
    sandbox.reverse(&mut history, Direction::Undo);
    assert!(sandbox.path("New folder").is_dir());
    assert!(!sandbox.path("Plans").exists());

    assert_eq!(history.next(Direction::Undo), Some(Kind::NewFolder));
    sandbox.reverse(&mut history, Direction::Undo);
    assert!(!sandbox.path("New folder").exists());

    sandbox.reverse(&mut history, Direction::Redo);
    sandbox.reverse(&mut history, Direction::Redo);
    assert!(sandbox.path("Plans").is_dir());
    assert_eq!(history.next(Direction::Redo), None);
}
