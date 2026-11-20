use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use rikka_open::{path_to_uri, uri_to_path};

#[test]
fn path_uri_round_trips_any_bytes() {
    let path = Path::new("/home/a/city horizon #2 100%.png");
    let uri = path_to_uri(path);
    assert_eq!(uri, "file:///home/a/city%20horizon%20%232%20100%25.png");
    assert_eq!(uri_to_path(&uri).as_deref(), Some(path));

    let raw = PathBuf::from(OsString::from_vec(b"/tmp/\xff\xfe".to_vec()));
    assert_eq!(uri_to_path(&path_to_uri(&raw)), Some(raw));
}

#[test]
fn only_local_file_uris_become_paths() {
    let here = Some(PathBuf::from("/home/a/b.png"));
    for uri in [
        "file:///home/a/b.png",
        "file://localhost/home/a/b.png",
        "FILE://LOCALHOST/home/a/b.png",
    ] {
        assert_eq!(uri_to_path(uri), here, "{uri}");
    }
    for uri in [
        "https://example.invalid/a.png",
        "file://elsewhere/a.png",
        "/home/a/b.png",
    ] {
        assert_eq!(uri_to_path(uri), None, "{uri}");
    }
}
