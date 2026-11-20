use files_core::{Entry, Location};
use rikka_file_operations::EntryKind;
use rikka_mime::MimeType;

fn entry(mime: &str, mode: u32) -> Entry {
    Entry {
        location: Location::from_path("/tmp/downloaded"),
        name: "downloaded".to_owned(),
        kind: EntryKind::File,
        mime: MimeType::parse(mime).expect("a type"),
        size: 1,
        modified: None,
        created: None,
        mode,
        hidden: false,
        link: None,
    }
}

#[test]
fn only_executable_programs_may_be_run() {
    let cases = [
        ("application/x-shellscript", 0o755, true),
        ("application/x-executable", 0o755, true),
        ("application/x-pie-executable", 0o700, true),
        ("application/vnd.appimage", 0o755, true),
        ("application/x-shellscript", 0o644, false),
        ("application/x-executable", 0o644, false),
        ("application/x-desktop", 0o755, false),
        ("text/plain", 0o755, false),
        ("image/png", 0o777, false),
        ("application/pdf", 0o755, false),
    ];
    for (mime, mode, runs) in cases {
        assert_eq!(
            files_open::is_program(&entry(mime, mode)),
            runs,
            "{mime} {mode:o}"
        );
    }
}
