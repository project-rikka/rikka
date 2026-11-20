use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use rikka_file_operations::{Phase, Placement, Resolution, Task};

use crate::support::{Elsewhere, Gate, Sandbox, finish, read, run, wait_for_conflict};

#[test]
fn copy_preserves_tree() {
    let sandbox = Sandbox::new();
    sandbox.file("src/one.txt", b"one");
    sandbox.file("src/deep/two.txt", b"two");
    sandbox.dir("dst");

    let outcome = finish(Task::Copy {
        sources: vec![sandbox.at("src")],
        into: sandbox.at("dst"),
    });

    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert_eq!(read(&sandbox.at("dst/src/one.txt")), b"one");
    assert_eq!(read(&sandbox.at("dst/src/deep/two.txt")), b"two");
    assert!(sandbox.at("src/one.txt").exists(), "source kept");
}

#[test]
fn move_removes_source() {
    let sandbox = Sandbox::new();
    sandbox.file("src/one.txt", b"one");
    sandbox.file("src/deep/two.txt", b"two");
    sandbox.dir("dst");

    let outcome = finish(Task::Move {
        sources: vec![sandbox.at("src")],
        into: sandbox.at("dst"),
    });

    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert_eq!(read(&sandbox.at("dst/src/deep/two.txt")), b"two");
    assert!(!sandbox.at("src").exists(), "source removed");
}

#[test]
fn same_named_folders_merge() {
    for moving in [false, true] {
        let sandbox = Sandbox::new();
        sandbox.file("src/shared/new.txt", b"new");
        sandbox.file("dst/src/shared/old.txt", b"old");
        let (sources, into) = (vec![sandbox.at("src")], sandbox.at("dst"));

        let outcome = finish(match moving {
            true => Task::Move { sources, into },
            false => Task::Copy { sources, into },
        });

        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
        assert_eq!(read(&sandbox.at("dst/src/shared/old.txt")), b"old");
        assert_eq!(read(&sandbox.at("dst/src/shared/new.txt")), b"new");
        assert_eq!(!sandbox.at("src").exists(), moving, "moving {moving}");
    }
}

#[test]
fn keep_places_arrival_beside_existing() {
    let sandbox = Sandbox::new();
    sandbox.file("src/note.txt", b"arriving");
    sandbox.file("dst/note.txt", b"already here");

    let operation = run(Task::Copy {
        sources: vec![sandbox.at("src/note.txt")],
        into: sandbox.at("dst"),
    });

    let question = wait_for_conflict(&operation);
    assert_eq!(question.source.name, "note.txt");
    assert_eq!(question.existing.name, "note.txt");
    assert_eq!(operation.snapshot().progress.phase, Phase::Asking);

    operation.resolve(Resolution::Keep, false);
    let outcome = operation.wait();

    assert_eq!(read(&sandbox.at("dst/note.txt")), b"already here");
    assert_eq!(read(&sandbox.at("dst/note (2).txt")), b"arriving");
    assert_eq!(
        outcome.placed,
        [Placement::new(
            sandbox.at("src/note.txt"),
            sandbox.at("dst/note (2).txt")
        )]
    );
}

#[test]
fn conflict_answers_decide_target() {
    let cases = [
        (Resolution::Replace, b"arriving".as_slice()),
        (Resolution::Skip, b"already here".as_slice()),
    ];
    for (answer, expected) in cases {
        let sandbox = Sandbox::new();
        sandbox.file("src/note.txt", b"arriving");
        sandbox.file("dst/note.txt", b"already here");

        let operation = run(Task::Copy {
            sources: vec![sandbox.at("src/note.txt")],
            into: sandbox.at("dst"),
        });
        wait_for_conflict(&operation);
        operation.resolve(answer, false);
        operation.wait();

        assert_eq!(read(&sandbox.at("dst/note.txt")), expected, "{answer:?}");
        assert_eq!(read(&sandbox.at("src/note.txt")), b"arriving", "{answer:?}");
        assert_eq!(sandbox.names("dst"), ["note.txt"], "{answer:?}");
    }
}

#[test]
fn standing_answer_settles_conflicts_before_they_arise() {
    let sandbox = Sandbox::new();
    sandbox.file("src/a.txt", b"new a");
    sandbox.file("src/b.txt", b"new b");
    sandbox.file("dst/a.txt", b"old a");
    sandbox.file("dst/b.txt", b"old b");

    let operation = run(Task::Copy {
        sources: vec![sandbox.at("src/a.txt"), sandbox.at("src/b.txt")],
        into: sandbox.at("dst"),
    });
    operation.resolve(Resolution::Keep, true);
    let outcome = operation.wait();

    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert_eq!(
        sandbox.names("dst"),
        ["a (2).txt", "a.txt", "b (2).txt", "b.txt"]
    );
}

#[test]
fn failures_collected_batch_continues() {
    let sandbox = Sandbox::new();
    sandbox.file("src/good.txt", b"good");
    sandbox.dir("dst");
    let missing = sandbox.at("src/never-existed.txt");

    let outcome = finish(Task::Copy {
        sources: vec![missing.clone(), sandbox.at("src/good.txt")],
        into: sandbox.at("dst"),
    });

    assert_eq!(read(&sandbox.at("dst/good.txt")), b"good");
    assert!(
        matches!(outcome.failures.as_slice(), [failure] if failure.at == missing),
        "{:?}",
        outcome.failures
    );
}

#[test]
fn cancel_stops_batch() {
    let sandbox = Sandbox::new();
    for index in 0..64 {
        sandbox.file(&format!("src/{index}.bin"), &vec![b'x'; 256 * 1024]);
    }
    sandbox.dir("dst");

    let operation = run(Task::Copy {
        sources: vec![sandbox.at("src")],
        into: sandbox.at("dst"),
    });
    operation.cancel();

    assert!(operation.wait().cancelled);
}

#[test]
fn cancel_mid_copy_leaves_no_partial_file() {
    let sandbox = Sandbox::new();
    let contents = vec![b'x'; 1024 * 1024];
    sandbox.file("src/big.bin", &contents);
    sandbox.file("dst/big.bin", b"the old one");
    let (gate, gated) = Gate::new();

    let operation = rikka_file_operations::run(
        Arc::new(gate),
        Task::Copy {
            sources: vec![sandbox.at("src/big.bin")],
            into: sandbox.at("dst"),
        },
    );
    operation.resolve(Resolution::Replace, true);
    gated.reading.recv().expect("the copy reads");
    operation.cancel();
    gated.resume.send(()).expect("the copy waits");
    let outcome = operation.wait();

    assert!(outcome.cancelled);
    assert_eq!(sandbox.names("dst"), ["big.bin"]);
    assert_eq!(read(&sandbox.at("dst/big.bin")), b"the old one");
    assert_eq!(read(&sandbox.at("src/big.bin")), contents);
}

#[test]
fn moving_across_devices_copies_then_removes() {
    let sandbox = Sandbox::new();
    sandbox.file("src/notes/one.txt", b"one");
    std::os::unix::fs::symlink("one.txt", sandbox.at("src/notes/link")).expect("a link");
    let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    let file = std::fs::File::options()
        .write(true)
        .open(sandbox.at("src/notes/one.txt"))
        .expect("the file");
    file.set_modified(modified).expect("a time");
    sandbox.dir("dst");

    let outcome = rikka_file_operations::run(
        Arc::new(Elsewhere),
        Task::Move {
            sources: vec![sandbox.at("src/notes")],
            into: sandbox.at("dst"),
        },
    )
    .wait();

    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert!(!sandbox.at("src/notes").exists(), "source removed");
    assert_eq!(read(&sandbox.at("dst/notes/one.txt")), b"one");
    assert_eq!(
        std::fs::read_link(sandbox.at("dst/notes/link")).expect("still a link"),
        Path::new("one.txt")
    );
    let copied = std::fs::metadata(sandbox.at("dst/notes/one.txt")).expect("the copy");
    assert_eq!(copied.modified().expect("a time"), modified);
    assert_eq!(
        outcome.placed,
        [Placement::new(
            sandbox.at("src/notes"),
            sandbox.at("dst/notes")
        )]
    );
}

#[test]
fn copy_into_itself_rejected() {
    let sandbox = Sandbox::new();
    sandbox.file("src/one.txt", b"one");
    sandbox.dir("src/inner");

    let outcome = finish(Task::Copy {
        sources: vec![sandbox.at("src")],
        into: sandbox.at("src/inner"),
    });

    assert_eq!(outcome.failures.len(), 1);
    assert!(!sandbox.at("src/inner/src").exists());
}

#[test]
fn copy_into_own_folder_keeps_original() {
    let sandbox = Sandbox::new();
    sandbox.file("note.txt", b"only copy");
    sandbox.file("album/track.txt", b"track");

    let outcome = finish(Task::Copy {
        sources: vec![sandbox.at("note.txt"), sandbox.at("album")],
        into: sandbox.path().to_owned(),
    });

    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert_eq!(read(&sandbox.at("note.txt")), b"only copy");
    assert_eq!(read(&sandbox.at("note (2).txt")), b"only copy");
    assert_eq!(read(&sandbox.at("album (2)/track.txt")), b"track");
    assert!(!sandbox.at("album/track (2).txt").exists());
}

#[test]
fn move_into_own_folder_changes_nothing() {
    let sandbox = Sandbox::new();
    sandbox.file("note.txt", b"only copy");

    let outcome = finish(Task::Move {
        sources: vec![sandbox.at("note.txt")],
        into: sandbox.path().to_owned(),
    });

    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert!(outcome.placed.is_empty());
    assert_eq!(sandbox.names(""), ["note.txt"]);
}

#[test]
fn copy_keeps_symlinks() {
    let sandbox = Sandbox::new();
    sandbox.file("src/real.txt", b"real");
    sandbox.dir("src/folder");
    let links = [
        ("real.txt", "to-file"),
        ("folder", "to-folder"),
        ("nowhere", "broken"),
    ];
    for (target, link) in links {
        std::os::unix::fs::symlink(target, sandbox.at("src").join(link)).expect("a link");
    }
    sandbox.dir("dst");

    let outcome = finish(Task::Copy {
        sources: vec![sandbox.at("src")],
        into: sandbox.at("dst"),
    });

    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    for (target, link) in links {
        let copied = sandbox.at("dst/src").join(link);
        assert_eq!(
            std::fs::read_link(&copied).expect("still a link"),
            Path::new(target),
            "{link}"
        );
    }
}

#[test]
fn replace_refused_by_folder_keeps_no_draft() {
    let sandbox = Sandbox::new();
    sandbox.file("src/note.txt", b"arriving");
    sandbox.file("dst/note.txt/inside.txt", b"a folder in the way");

    let operation = run(Task::Copy {
        sources: vec![sandbox.at("src/note.txt")],
        into: sandbox.at("dst"),
    });
    wait_for_conflict(&operation);
    operation.resolve(Resolution::Replace, false);
    let outcome = operation.wait();

    assert_eq!(outcome.failures.len(), 1);
    assert_eq!(sandbox.names("dst"), ["note.txt"]);
    assert_eq!(
        read(&sandbox.at("dst/note.txt/inside.txt")),
        b"a folder in the way"
    );
}

#[test]
fn delete_removes_tree_without_following_links() {
    let sandbox = Sandbox::new();
    sandbox.file("kept/precious.txt", b"precious");
    sandbox.file("doomed/inner/file.txt", b"gone");
    std::os::unix::fs::symlink(sandbox.at("kept"), sandbox.at("doomed/inner/to-kept"))
        .expect("a link");

    let outcome = finish(Task::Delete {
        items: vec![sandbox.at("doomed")],
    });

    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert!(!sandbox.at("doomed").exists());
    assert_eq!(read(&sandbox.at("kept/precious.txt")), b"precious");
}
