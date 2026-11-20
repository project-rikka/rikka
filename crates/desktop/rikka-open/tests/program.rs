use std::path::Path;

use rikka_open::{Error, find, is_executable, is_runnable, open};

#[test]
fn runnable_requires_executable() {
    assert!(is_runnable("sh"), "$PATH is searched by name");
    for program in [
        "rikka-no-such-program-anywhere",
        "/etc/hostname",
        "/no/such/path",
        "",
    ] {
        assert!(!is_runnable(program), "{program}");
    }
    assert!(!is_executable(Path::new("/usr")));

    let found = find("sh").expect("$PATH has a shell");
    assert!(found.is_absolute());
    assert_eq!(found.file_name().and_then(|name| name.to_str()), Some("sh"));
}

#[test]
fn rejects_empty_open_target() {
    assert!(matches!(open("  "), Err(Error::NothingToOpen)));
}
