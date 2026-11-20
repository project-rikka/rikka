use std::path::Path;
use std::process::Command;

use ade_git::{Draft, Error};

use crate::support::{git, repository, tracked, write};

#[test]
fn commits_only_selected_files() {
    let Some((folder, repo)) = repository() else {
        return;
    };
    let root = folder.path();
    write(root, "src/main.rs", "fn main() {\n    let one = 2;\n}\n");
    write(root, "notes.txt", "a new file\n");

    let status = repo.status().expect("git says what changed");
    let commit = Draft::new("edit main")
        .description("the body of it")
        .with_changes(
            status
                .changes()
                .iter()
                .map(|change| (change, change.path() == Path::new("src/main.rs"))),
        );
    repo.commit(&commit).expect("the commit is made");

    let after = repo.status().expect("git says what is left");
    let left: Vec<&Path> = after.changes().iter().map(ade_git::Change::path).collect();
    assert_eq!(left, [Path::new("notes.txt")]);

    let output = Command::new("git")
        .current_dir(root)
        .args(["log", "-1", "--pretty=%s%n%n%b"])
        .output()
        .expect("git says what was committed");
    let content = String::from_utf8_lossy(&output.stdout);
    assert!(content.starts_with("edit main\n"), "{content}");
    assert!(content.contains("the body of it"), "{content}");
}

#[test]
fn unselecting_unstages_file() {
    let Some((folder, repo)) = repository() else {
        return;
    };
    let root = folder.path();
    write(root, "src/main.rs", "fn main() {\n    let one = 2;\n}\n");
    write(root, "notes.txt", "a new file\n");
    git(root, &["add", "--all"]).expect("both are staged");

    let status = repo.status().expect("git says what changed");
    let commit = Draft::new("edit main").with_changes(
        status
            .changes()
            .iter()
            .map(|change| (change, change.path() == Path::new("src/main.rs"))),
    );
    repo.commit(&commit).expect("the commit is made");

    let after = repo.status().expect("git says what is left");
    assert_eq!(
        after
            .changes()
            .iter()
            .map(ade_git::Change::path)
            .collect::<Vec<_>>(),
        [Path::new("notes.txt")]
    );
}

#[test]
fn rejects_empty_commit() {
    let Some((folder, repo)) = repository() else {
        return;
    };
    write(folder.path(), "src/main.rs", "fn main() {}\n");
    let status = repo.status().expect("git says what changed");
    let ticked = || status.changes().iter().map(|change| (change, true));

    assert!(matches!(
        repo.commit(&Draft::new("  ").with_changes(ticked())),
        Err(Error::Unsummarised)
    ));
    assert!(matches!(
        repo.commit(&Draft::new("says nothing")),
        Err(Error::Nothing)
    ));
}

#[test]
fn glob_like_name_is_literal() {
    let Some((folder, repo)) = repository() else {
        return;
    };
    let root = folder.path();
    write(root, "note[s].txt", "a new file\n");
    write(root, "notes.txt", "another new file\n");

    let status = repo.status().expect("git says what changed");
    let commit = Draft::new("write one down").with_changes(
        status
            .changes()
            .iter()
            .map(|change| (change, change.path() == Path::new("note[s].txt"))),
    );
    repo.commit(&commit).expect("the commit is made");

    assert_eq!(tracked(root), [".gitignore", "note[s].txt", "src/main.rs"]);
    let after = repo.status().expect("git says what is left");
    assert_eq!(
        after
            .changes()
            .iter()
            .map(ade_git::Change::path)
            .collect::<Vec<_>>(),
        [Path::new("notes.txt")]
    );
}
