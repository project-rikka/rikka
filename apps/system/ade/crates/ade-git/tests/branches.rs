use std::path::Path;

use ade_git::Kind;

use crate::support::{git, repository, write};

#[test]
fn history_lists_commits_newest_first() {
    let Some((folder, repo)) = repository() else {
        return;
    };
    let root = folder.path();
    write(root, "src/main.rs", "fn main() {\n    let one = 2;\n}\n");
    write(root, "notes.txt", "a new file\n");
    git(root, &["add", "--all"]).expect("both are staged");
    git(
        root,
        &["commit", "--quiet", "-m", "second", "-m", "why it was done"],
    )
    .expect("a second commit");

    let history = repo.history().expect("git says what was committed");
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].summary(), "second");
    assert_eq!(history[0].body(), "why it was done");
    assert_eq!(history[1].summary(), "first");
    assert_eq!(history[0].author(), "ADE");
    assert!(history[0].at() >= history[1].at());
    assert!(history[0].oid().starts_with(history[0].short()));

    let patch = repo.patch(history[0].oid()).expect("what it changed");
    let changed: Vec<&Path> = patch.files().iter().map(ade_git::Diff::path).collect();
    assert_eq!(changed, [Path::new("notes.txt"), Path::new("src/main.rs")]);
    assert_eq!((patch.added(), patch.removed()), (2, 1));
}

#[test]
fn switch_checks_out_branch() {
    let Some((folder, repo)) = repository() else {
        return;
    };
    let root = folder.path();
    git(root, &["branch", "spare"]).expect("a second branch");

    let status = repo.status().expect("git says where it is");
    let worktrees = repo.worktrees();
    let branches = repo.branches(&status, &worktrees);
    let names: Vec<(&str, bool)> = branches
        .iter()
        .map(|branch| (branch.name(), branch.is_head()))
        .collect();
    assert_eq!(names, [("main", true), ("spare", false)]);
    assert!(branches.iter().all(|branch| branch.kind() == Kind::Local));
    assert!(
        branches
            .iter()
            .all(|branch| !branch.is_checked_out_elsewhere())
    );

    let spare = branches
        .iter()
        .find(|branch| branch.name() == "spare")
        .expect("it is offered");
    repo.switch(spare).expect("it is checked out");

    let after = repo.status().expect("git says where it is now");
    assert_eq!(after.branch(), Some("spare"));
}
