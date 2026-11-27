use rikka_file_edit::Replacement;

use crate::support::Project;

#[test]
fn write_creates_parents_and_keeps_line_endings() {
    let project = Project::open();

    let wrote = project
        .files
        .write("src/deep/new.rs", "a\nb\n")
        .expect("written");
    assert_eq!(project.read_file("src/deep/new.rs"), "a\nb\n");
    assert_eq!((wrote.lines, wrote.created), (2, true));

    project.write_file("windows.txt", "one\r\n");
    let wrote = project
        .files
        .write("windows.txt", "two\nthree\n")
        .expect("written");
    assert_eq!(project.read_file("windows.txt"), "two\r\nthree\r\n");
    assert!(!wrote.created);
}

#[test]
fn rejects_paths_outside_workspace() {
    let project = Project::open();
    let elsewhere = tempfile::tempdir().expect("somewhere else");
    std::os::unix::fs::symlink(elsewhere.path(), project.folder.path().join("away"))
        .expect("the link is made");
    std::fs::create_dir(project.folder.path().join("src")).expect("the folder is made");

    assert!(
        project
            .edit("../escaped.rs", vec![Replacement::new("a", "b")])
            .is_err()
    );
    assert!(project.files.write("../escaped.rs", "x").is_err());
    assert!(project.files.write("/etc/escaped.rs", "x").is_err());
    assert!(project.files.write("away/file.rs", "x").is_err());
    assert!(!elsewhere.path().join("file.rs").exists());
    assert!(
        project.files.write("src", "x").is_err(),
        "a folder is not written over"
    );

    std::os::unix::fs::symlink(
        elsewhere.path().join("absent.rs"),
        project.folder.path().join("dangling.rs"),
    )
    .expect("the link is made");
    assert!(project.files.write("dangling.rs", "x").is_err());
    assert!(!elsewhere.path().join("absent.rs").exists());
}
