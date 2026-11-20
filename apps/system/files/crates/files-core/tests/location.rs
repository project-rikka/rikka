use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

use files_core::Location;

#[test]
fn path_and_uri_round_trip() {
    let location = Location::from_path("/home/eldo/写真");

    assert_eq!(location.as_str(), "file:///home/eldo/%E5%86%99%E7%9C%9F");
    assert_eq!(location.to_path(), Some(PathBuf::from("/home/eldo/写真")));
    assert_eq!(Location::parse(location.as_str()), Some(location));
}

#[test]
fn non_utf8_name_round_trips() {
    let raw = PathBuf::from(OsString::from_vec(b"/tmp/\xff\xfename".to_vec()));

    let location = Location::from_path(&raw);

    assert_eq!(location.to_path(), Some(raw.clone()));
    assert_eq!(
        Location::parse(location.as_str()).and_then(|parsed| parsed.to_path()),
        Some(raw)
    );
}

#[test]
fn uri_normalizes_to_one_form() {
    let docs = Location::from_path("/home/eldo/docs");

    for spelled in [
        "file:/home/eldo/docs",
        "file:///home/eldo/docs",
        "file://localhost/home/eldo/docs",
        "file:///home/eldo/../eldo/./docs/",
    ] {
        assert_eq!(Location::parse(spelled).as_ref(), Some(&docs), "{spelled}");
    }
}

#[test]
fn contains_matches_whole_segments() {
    let folder = Location::from_path("/home/a");

    assert!(folder.contains(&Location::from_path("/home/a/deep/file")));
    assert!(!folder.contains(&Location::from_path("/home/ab")));
}

#[test]
fn parse_rejects_non_uri() {
    assert_eq!(Location::parse("/home/eldo"), None);
    assert_eq!(Location::parse("relative/path"), None);
    assert_eq!(Location::parse("12:30 notes.txt"), None);
    assert_eq!(Location::parse("file://elsewhere/home/eldo"), None);
}
