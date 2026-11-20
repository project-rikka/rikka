use std::io::Write;
use std::os::unix::fs::PermissionsExt;

use rikka_fs::{Replace, replace};

fn mode(path: &std::path::Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

#[test]
fn replace_creates_parents_and_keeps_mode() {
    let folder = tempfile::tempdir().unwrap();
    let fresh = folder.path().join("deep/er/fresh.toml");
    replace(&fresh, "one").unwrap();
    assert_eq!(std::fs::read_to_string(&fresh).unwrap(), "one");
    assert_eq!(mode(&fresh), 0o644);

    std::fs::set_permissions(&fresh, std::fs::Permissions::from_mode(0o600)).unwrap();
    replace(&fresh, b"two").unwrap();
    assert_eq!(std::fs::read_to_string(&fresh).unwrap(), "two");
    assert_eq!(mode(&fresh), 0o600);

    let left: Vec<_> = std::fs::read_dir(fresh.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(left, ["fresh.toml"]);
}

#[test]
fn private_replace_sets_owner_only_mode() {
    let folder = tempfile::tempdir().unwrap();
    let secret = folder.path().join("secret.toml");
    Replace::new(&secret).private().write("key").unwrap();
    assert_eq!(mode(&secret), 0o600);

    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o644)).unwrap();
    Replace::new(&secret).private().write("other").unwrap();
    assert_eq!(mode(&secret), 0o600);
    assert_eq!(std::fs::read_to_string(&secret).unwrap(), "other");
}

#[test]
fn default_mode_applies_only_to_new_files() {
    let folder = tempfile::tempdir().unwrap();
    let shared = folder.path().join("shared.toml");
    Replace::new(&shared)
        .default_mode(0o640)
        .write("one")
        .unwrap();
    assert_eq!(mode(&shared), 0o640);

    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o604)).unwrap();
    Replace::new(&shared)
        .default_mode(0o640)
        .write("two")
        .unwrap();
    assert_eq!(mode(&shared), 0o604);
}

#[test]
fn replace_writes_through_symlink() {
    let folder = tempfile::tempdir().unwrap();
    let real = folder.path().join("real.conf");
    std::fs::write(&real, "old").unwrap();
    let link = folder.path().join("link.conf");
    std::os::unix::fs::symlink("real.conf", &link).unwrap();

    replace(&link, "new").unwrap();

    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read_to_string(&real).unwrap(), "new");
}

#[test]
fn abandoned_stage_leaves_target_untouched() {
    let folder = tempfile::tempdir().unwrap();
    let kept = folder.path().join("kept.toml");
    std::fs::write(&kept, "old").unwrap();

    let mut staged = Replace::new(&kept).stage().unwrap();
    staged.write_all(b"half").unwrap();
    drop(staged);

    assert_eq!(std::fs::read_to_string(&kept).unwrap(), "old");
    let left: Vec<_> = std::fs::read_dir(folder.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(left, ["kept.toml"]);
}

#[test]
fn staged_file_written_by_path_commits_to_first_unused_name() {
    let folder = tempfile::tempdir().unwrap();
    let taken = folder.path().join("clip.mp4");
    std::fs::write(&taken, "first").unwrap();

    let staged = Replace::new(&taken).keep_extension().stage().unwrap();
    assert_eq!(staged.path().extension().unwrap(), "mp4");
    std::fs::write(staged.path(), "second").unwrap();
    let free = folder.path().join("clip (2).mp4");
    let landed = staged.commit_unused([taken.clone(), free.clone()]).unwrap();

    assert_eq!(landed, free);
    assert_eq!(std::fs::read_to_string(&taken).unwrap(), "first");
    assert_eq!(std::fs::read_to_string(&free).unwrap(), "second");
    assert_eq!(mode(&free), 0o644);
    assert_eq!(std::fs::read_dir(folder.path()).unwrap().count(), 2);
}
