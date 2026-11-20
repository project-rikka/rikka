use rikka_file_edit::{Error, TextFile};

#[test]
fn text_file_round_trips_line_endings() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = folder.path().join("notes.md");
    std::fs::write(&path, "\u{feff}one\r\ntwo\r\n").expect("a scratch file");

    let mut file = TextFile::read(&path).expect("a readable file");
    assert_eq!(file.content(), "one\ntwo\n");

    file.write("one\ntwo\nthree\n").expect("a write");
    file.write("one\ntwo\nthree\nfour\n")
        .expect("a second write follows the first");
    assert_eq!(
        std::fs::read(&path).expect("the file"),
        "\u{feff}one\r\ntwo\r\nthree\r\nfour\r\n".as_bytes()
    );
}

#[test]
fn text_file_refuses_concurrently_modified_file() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = folder.path().join("shared.txt");
    std::fs::write(&path, "mine\n").expect("a scratch file");

    let mut file = TextFile::read(&path).expect("a readable file");
    std::fs::write(&path, "theirs\n").expect("somebody else writes");

    assert!(file.write("ours\n").is_err());
    assert_eq!(
        std::fs::read_to_string(&path).expect("the file"),
        "theirs\n"
    );
}

#[test]
fn text_file_reports_missing_and_binary_files() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = folder.path().join("logo.png");
    std::fs::write(&path, [0x89, 0x50, 0x4e, 0x47, 0x00, 0x01]).expect("a scratch file");

    assert!(matches!(TextFile::read(&path), Err(Error::NotText(_))));
    assert!(matches!(
        TextFile::read(&folder.path().join("missing.txt")),
        Err(Error::NotFound(_))
    ));
}
