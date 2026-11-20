use rikka_xdg::KeyFile;

const FILE: &str = "\
# a comment
[Desktop Entry]
Type=Application
Name=Terminal
Name[ja]=端末
Keywords=shell;prompt;command line;
Terminal=false

[Desktop Action new-window]
Name=New Window
";

#[test]
fn key_belongs_to_its_group() {
    let file = KeyFile::parse(FILE);
    let entry = file.group("Desktop Entry").expect("the first group");

    assert_eq!(entry.raw("Type"), Some("Application"));
    assert_eq!(entry.string("Name").as_deref(), Some("Terminal"));
    assert!(!entry.boolean("Terminal"));
    assert_eq!(
        file.group("Desktop Action new-window")
            .and_then(|action| action.string("Name"))
            .as_deref(),
        Some("New Window"),
        "an action's name is its own"
    );
    assert!(file.group("Nothing").is_none());
}

#[test]
fn list_separator_can_be_escaped() {
    let file = KeyFile::parse("[Desktop Entry]\nKeywords=a\\;b;c\\\\;d\n");
    assert_eq!(
        file.group("Desktop Entry").unwrap().list("Keywords", &[]),
        ["a;b", "c\\", "d"]
    );
}

#[test]
fn invalid_line_is_skipped_alone() {
    let file = KeyFile::parse("[Desktop Entry]\nthis has no equals sign\nName=Kept\n");
    assert_eq!(
        file.group("Desktop Entry").unwrap().raw("Name"),
        Some("Kept")
    );
    assert!(
        KeyFile::parse("Name=orphan\n")
            .group("Desktop Entry")
            .is_none(),
        "a key before any group belongs to none"
    );
}
