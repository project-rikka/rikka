use std::fs;
use std::path::{Path, PathBuf};

use ade_files::{Change, Error, Kind, Moved, Root};
use rikka_file_operations::Resolution;
use tempfile::TempDir;

fn project() -> (TempDir, Root) {
    let dir = TempDir::new().expect("a directory");
    let root = Root::new(dir.path());
    (dir, root)
}

fn made(root: &Root, folder: &Path, typed: &str, kind: Kind) -> PathBuf {
    root.create(folder, typed, kind)
        .expect("something made")
        .path
}

#[test]
fn create_makes_parent_folders() {
    let (dir, root) = project();
    let made = root
        .create(dir.path(), "src/deep/mod.rs", Kind::File)
        .expect("a new file");

    assert_eq!(made.path, dir.path().join("src/deep/mod.rs"));
    assert_eq!(made.kind, Kind::File);
    assert!(made.path.is_file());
    assert!(matches!(
        root.create(dir.path(), "src/deep/mod.rs", Kind::File),
        Err(Error::Exists(_))
    ));
}

#[test]
fn operations_stay_inside_project() {
    let (dir, root) = project();
    let elsewhere = TempDir::new().expect("another directory");
    let outside = elsewhere.path().join("x.rs");
    fs::write(&outside, "outside").expect("a file");

    assert!(matches!(
        root.create(elsewhere.path(), "x", Kind::File),
        Err(Error::Outside(_))
    ));
    assert!(matches!(
        root.delete(&[dir.path().to_owned()]),
        Err(Error::Outside(_))
    ));
    assert!(matches!(
        root.move_into(std::slice::from_ref(&outside), dir.path()),
        Err(Error::Outside(_))
    ));
    assert!(matches!(
        root.create(dir.path(), "../x", Kind::File),
        Err(Error::BadName(_))
    ));
    assert_eq!(fs::read_to_string(&outside).expect("untouched"), "outside");
}

#[test]
fn clashing_names_keep_both() {
    let (dir, root) = project();
    let arriving = made(&root, dir.path(), "a.rs", Kind::File);
    fs::write(&arriving, "arriving").expect("a written file");
    let folder = made(&root, dir.path(), "src", Kind::Folder);
    let resident = made(&root, &folder, "a.rs", Kind::File);
    fs::write(&resident, "resident").expect("a written file");

    let kept_both = |job: ade_files::Job| {
        job.resolve(Resolution::Keep, true);
        job.wait()
    };
    let moved = kept_both(
        root.move_into(std::slice::from_ref(&arriving), &folder)
            .expect("a move"),
    );
    let copied = kept_both(
        root.copy_into(std::slice::from_ref(&resident), &folder)
            .expect("a copy"),
    );

    assert!(moved.failures.is_empty(), "{:?}", moved.failures);
    assert_eq!(
        moved.change,
        Change::Moved(vec![Moved {
            from: arriving,
            to: folder.join("a (2).rs"),
        }])
    );
    assert_eq!(copied.change, Change::Copied(vec![folder.join("a (3).rs")]));
    assert_eq!(fs::read_to_string(&resident).expect("kept"), "resident");
    assert_eq!(
        fs::read_to_string(folder.join("a (2).rs")).expect("moved"),
        "arriving"
    );
}

#[test]
fn batch_reports_failure_and_keeps_going() {
    let (dir, root) = project();
    let missing = dir.path().join("missing.rs");
    let present = made(&root, dir.path(), "b.rs", Kind::File);
    let folder = made(&root, dir.path(), "src", Kind::Folder);

    let moved = root
        .move_into(&[missing, present.clone()], &folder)
        .expect("a move")
        .wait();

    assert_eq!(
        moved.change,
        Change::Moved(vec![Moved {
            from: present,
            to: folder.join("b.rs"),
        }])
    );
    assert!(matches!(moved.failures.as_slice(), [Error::Operation(_)]));
    assert!(!moved.cancelled);
}

#[test]
fn delete_reports_what_it_removed() {
    let (dir, root) = project();
    let file = made(&root, dir.path(), "a.rs", Kind::File);
    let folder = made(&root, dir.path(), "src", Kind::Folder);
    made(&root, &folder, "deep/b.rs", Kind::File);
    let missing = dir.path().join("missing.rs");
    let kept = made(&root, dir.path(), "kept.rs", Kind::File);

    let deleted = root
        .delete(&[file.clone(), folder.clone(), missing.clone()])
        .expect("a deletion")
        .wait();

    assert_eq!(deleted.change, Change::Removed(vec![file, folder, missing]));
    assert!(kept.exists());
}
