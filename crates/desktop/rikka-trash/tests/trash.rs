use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use rikka_trash::{Error, entries, erase, restore, trash};

fn trashed_originals() -> Vec<PathBuf> {
    let mut originals: Vec<PathBuf> = entries().into_iter().map(|entry| entry.original).collect();
    originals.sort();
    originals
}

#[test]
fn trash_records_info_and_restores() {
    let home = tempfile::tempdir().expect("a temporary home");
    let data = home.path().join("data");
    std::fs::create_dir(&data).expect("the data home is made");
    unsafe { std::env::set_var("XDG_DATA_HOME", &data) };
    let trash_dir = data.join("Trash");
    std::fs::create_dir(&trash_dir).expect("trash directory is created");

    let pictures = home.path().join("写真");
    std::fs::create_dir(&pictures).expect("a folder of pictures");
    let tree = pictures.join("桜 の a#b.png");
    let raw = pictures.join(OsString::from_vec(b"\xff\xfename".to_vec()));
    for (path, bytes) in [(&tree, b"tree".as_slice()), (&raw, b"raw".as_slice())] {
        std::fs::create_dir(path).expect("folder to trash is created");
        std::fs::write(path.join("inside"), bytes).expect("what it holds");
    }

    let sent = trash(&tree).expect("the tree is trashed");
    trash(&raw).expect("a name that is not UTF-8 is trashed");
    assert!(!tree.exists());
    assert_eq!(sent.parent(), Some(trash_dir.join("files").as_path()));

    let record = std::fs::read_to_string(trash_dir.join("info").join("桜 の a#b.png.trashinfo"))
        .expect("the record sits beside it");
    assert!(record.starts_with("[Trash Info]\n"), "{record}");
    assert!(
        record.contains("%E6%A1%9C%20%E3%81%AE%20a%23b.png"),
        "{record}"
    );
    assert!(
        record.contains("Path=/"),
        "home trash records absolute paths: {record}"
    );
    assert!(record.contains("DeletionDate="), "{record}");
    assert_eq!(trashed_originals(), {
        let mut both = vec![tree.clone(), raw.clone()];
        both.sort();
        both
    });
    assert!(entries().iter().all(|entry| entry.deleted.is_some()));

    std::fs::create_dir(&tree).expect("a second folder of the same name");
    let second = trash(&tree).expect("the second tree is trashed");
    assert_ne!(second, sent, "a second folder never lands on the first");
    assert_eq!(
        std::fs::read(sent.join("inside")).expect("the first is kept"),
        b"tree"
    );

    assert_eq!(restore(&sent).expect("it comes back"), tree);
    assert_eq!(
        std::fs::read(tree.join("inside")).expect("restored"),
        b"tree"
    );
    assert!(matches!(restore(&second), Err(Error::Exists(_))));
    erase(&second).expect("the second is erased");

    let kept = pictures.join("inside");
    std::fs::write(&kept, b"kept").expect("a file named like one inside a trashed folder");
    trash(&kept).expect("it is trashed");
    trash(&tree).expect("the restored tree is trashed again");
    let nested = trash_dir.join("files").join("桜 の a#b.png").join("inside");
    erase(&nested).expect("a file inside a trashed folder is erased");
    assert!(!nested.exists());
    assert!(
        trashed_originals().contains(&kept),
        "erasing a nested file must keep the record of its top-level namesake"
    );
    for entry in entries() {
        if entry.original == kept || entry.original == tree {
            erase(&entry.path).expect("it is erased");
        }
    }

    std::fs::write(trash_dir.join("files").join("stray.png"), b"stray").expect("no record");
    assert_eq!(trashed_originals(), [raw]);

    assert!(matches!(
        trash(Path::new("/nowhere/at/all/桜.png")),
        Err(Error::NotFound(_))
    ));

    let outside = pictures.join("survivor");
    std::fs::create_dir(&outside).expect("a folder outside every trash");
    assert!(matches!(erase(&outside), Err(Error::Unsupported(_))));
    assert!(matches!(
        erase(&trash_dir.join("files")),
        Err(Error::Unsupported(_))
    ));
    assert!(outside.exists() && trash_dir.join("files").exists());
}

#[test]
fn restore_finds_volume_trash_outside_mount_table() {
    let volume = tempfile::tempdir().expect("a volume");
    let uid = std::fs::metadata("/proc/self").expect("own process").uid();
    let store = volume.path().join(format!(".Trash-{uid}"));
    let trashed = store.join("files").join("plans");
    std::fs::create_dir_all(&trashed).expect("trashed folder");
    std::fs::write(trashed.join("draft.txt"), b"draft").expect("what it holds");
    std::fs::create_dir(store.join("info")).expect("info directory");
    let record = store.join("info").join("plans.trashinfo");
    std::fs::write(
        &record,
        "[Trash Info]\nPath=plans\nDeletionDate=2026-11-08T21:00:00\n",
    )
    .expect("record");

    let original = restore(&trashed).expect("it comes back");

    assert_eq!(original, volume.path().join("plans"));
    assert_eq!(
        std::fs::read(original.join("draft.txt")).expect("restored"),
        b"draft"
    );
    assert!(!record.exists());
}
