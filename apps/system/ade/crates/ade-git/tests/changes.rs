use std::path::Path;

use ade_git::{Line, Repo, Stat, State};

use crate::support::{git, repository, write};

#[test]
fn non_repository_is_rejected() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    if git(folder.path(), &["rev-parse", "--show-toplevel"]).is_some() {
        return;
    }
    assert!(Repo::discover(folder.path()).is_err());
}

#[test]
fn reads_every_change_kind() {
    let Some((folder, repo)) = repository() else {
        return;
    };
    let root = folder.path();
    write(root, "src/main.rs", "fn main() {\n    let one = 2;\n}\n");
    write(root, "notes.txt", "a new file\n");
    write(root, "target/build.log", "ignored\n");

    let status = repo.status().expect("git says what changed");
    assert_eq!(status.branch(), Some("main"));
    assert!(status.is_born());

    let states: Vec<(&Path, State)> = status
        .changes()
        .iter()
        .map(|change| (change.path(), change.state()))
        .collect();
    assert_eq!(
        states,
        [
            (Path::new("notes.txt"), State::Untracked),
            (Path::new("src/main.rs"), State::Modified),
        ],
        "what .gitignore keeps out is not a change"
    );

    let edited = repo
        .diff(&status.changes()[1], status.is_born())
        .expect("a diff of the edit");
    assert_eq!((edited.added(), edited.removed()), (1, 1));
    assert_eq!(edited.files()[0].path(), Path::new("src/main.rs"));
    assert!(edited.files()[0].lines().contains(&Line::Added {
        content: "    let one = 2;".to_owned(),
        at: 2
    }));

    let arrived = repo
        .diff(&status.changes()[0], status.is_born())
        .expect("a diff of the new file");
    assert_eq!((arrived.added(), arrived.removed()), (1, 0));
    assert_eq!(
        arrived.files()[0].path(),
        Path::new("notes.txt"),
        "a file git is not tracking is still named the way the project names it"
    );
}

#[test]
fn changes_report_line_counts() {
    let Some((folder, repo)) = repository() else {
        return;
    };
    let root = folder.path();
    write(
        root,
        "src/main.rs",
        "fn main() {\n    let one = 2;\n    let two = 2;\n}\n",
    );
    write(root, "notes.txt", "one\ntwo\nthree");
    std::fs::write(root.join("logo.png"), b"PNG\0\x01\x02").expect("a picture to write");
    std::fs::remove_file(root.join(".gitignore")).expect("the ignore file goes away");

    let status = repo.status().expect("git says what changed");
    let stats: Vec<(&Path, Option<Stat>)> = status
        .changes()
        .iter()
        .map(|change| (change.path(), change.stat()))
        .collect();
    assert_eq!(
        stats,
        [
            (Path::new(".gitignore"), Some(Stat::new(0, 1))),
            (Path::new("logo.png"), None),
            (Path::new("notes.txt"), Some(Stat::new(3, 0))),
            (Path::new("src/main.rs"), Some(Stat::new(2, 1))),
        ],
        "a picture has no lines to count"
    );
    assert_eq!(status.stat(), Stat::new(5, 2));
}

#[test]
fn conflicted_file_is_readable_change() {
    let Some((folder, repo)) = repository() else {
        return;
    };
    let root = folder.path();

    git(root, &["switch", "--quiet", "-c", "theirs"]).expect("a second branch");
    write(root, "src/main.rs", "fn main() {\n    let one = 3;\n}\n");
    git(root, &["commit", "--quiet", "-am", "theirs"]).expect("a commit on it");

    git(root, &["switch", "--quiet", "main"]).expect("back to main");
    write(root, "src/main.rs", "fn main() {\n    let one = 2;\n}\n");
    git(root, &["commit", "--quiet", "-am", "ours"]).expect("a commit on main");

    assert!(git(root, &["merge", "--no-edit", "theirs"]).is_none());

    let status = repo.status().expect("git says what changed");
    let conflicted = status
        .changes()
        .iter()
        .find(|change| change.state() == State::Conflicted)
        .expect("the merge left one");
    assert_eq!(conflicted.path(), Path::new("src/main.rs"));

    let patch = repo
        .diff(conflicted, status.is_born())
        .expect("a diff of the conflict");
    assert!(
        !patch.files().is_empty(),
        "a combined diff reads as nothing"
    );
    let content: Vec<&str> = patch.files()[0]
        .lines()
        .iter()
        .filter_map(|line| match line {
            Line::Added { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        content.iter().any(|line| line.starts_with(">>>>>>>")),
        "the markers in the file are what there is to see: {content:?}"
    );
}
