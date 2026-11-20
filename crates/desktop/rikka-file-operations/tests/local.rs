use rikka_file_operations::{Error, Local, Store};

use crate::support::{Sandbox, read};

#[test]
fn walk_lists_folder_entries() {
    let sandbox = Sandbox::new();
    Local.create_file(&sandbox.at("a.txt")).expect("a file");
    Local.create_directory(&sandbox.at("b")).expect("a folder");

    let mut listing = Local.walk(&sandbox.path().to_owned()).expect("a listing");
    listing.sort_by(|one, other| one.name.cmp(&other.name));

    let names: Vec<&str> = listing.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, ["a.txt", "b"]);
    assert!(listing[1].is_directory());
}

#[test]
fn rename_never_replaces() {
    let sandbox = Sandbox::new();
    let one = sandbox.file("one", b"one");
    sandbox.file("two", b"precious");
    let lower = sandbox.file("readme.md", b"lower");
    let upper = sandbox.file("README.md", b"upper");

    assert!(matches!(
        Local.rename(&one, "two"),
        Err(Error::Exists { .. })
    ));
    assert!(matches!(
        Local.rename(&upper, "readme.md"),
        Err(Error::Exists { .. })
    ));
    assert!(matches!(
        Local.rename(&one, "../escaped"),
        Err(Error::BadName { .. })
    ));
    assert_eq!(read(&sandbox.at("two")), b"precious");
    assert_eq!(read(&lower), b"lower");
    assert_eq!(read(&upper), b"upper");
}

#[test]
fn rename_changes_case() {
    let sandbox = Sandbox::new();
    let one = sandbox.file("one.rs", b"one");

    let renamed = Local.rename(&one, "One.rs").expect("a new case");

    assert_eq!(renamed, sandbox.at("One.rs"));
    assert_eq!(read(&renamed), b"one");
}

#[test]
fn delete_symlink_keeps_target() {
    let sandbox = Sandbox::new();
    sandbox.file("real/inside", b"x");
    std::os::unix::fs::symlink(sandbox.at("real"), sandbox.at("link")).expect("a link");

    Local.delete(&sandbox.at("link")).expect("the link deleted");

    assert!(sandbox.at("real/inside").exists());
    assert!(sandbox.at("link").symlink_metadata().is_err());
}
