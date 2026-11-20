use std::fs;

use rikka_i18n::load_dir;

#[test]
fn load_dir_reports_broken_files() {
    let dir = tempfile::tempdir().expect("a directory");
    fs::write(dir.path().join("en.toml"), "greeting = \"hi\"").unwrap();
    fs::write(dir.path().join("ja.toml"), "greeting = ").unwrap();
    fs::write(dir.path().join("notes.txt"), "not a catalog").unwrap();

    let report = load_dir(dir.path());
    assert_eq!(report.loaded.len(), 1);
    assert_eq!(report.loaded[0].locale().to_string(), "en");
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].path, dir.path().join("ja.toml"));

    let missing = load_dir(dir.path().join("does-not-exist"));
    assert!(missing.loaded.is_empty() && missing.errors.is_empty());
}
