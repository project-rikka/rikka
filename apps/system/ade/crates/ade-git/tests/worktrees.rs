use crate::support::repository;

#[test]
fn worktree_is_created_and_excluded() {
    let Some((folder, repo)) = repository() else {
        return;
    };
    let root = folder.path();

    let made = repo
        .add_worktree("feature/one", Some("main"))
        .expect("a worktree on a new branch");

    assert_eq!(made.path(), root.join(".rikka/worktrees/feature-one"));
    assert_eq!(made.branch(), Some("feature/one"));
    assert!(made.path().join("src/main.rs").is_file());
    assert!(
        std::fs::read_to_string(root.join(".git/info/exclude"))
            .expect("the repository excludes what it keeps")
            .lines()
            .any(|line| line == "/.rikka/worktrees/"),
        "worktrees directory must not show in status"
    );
    assert!(
        !root.join(".rikka/worktrees/.gitignore").exists(),
        "an ignore file inside would hide the worktree from every walker"
    );

    let status = repo.status().expect("git says what changed");
    assert!(
        status.changes().is_empty(),
        "a worktree of its own project is not a change in it: {:?}",
        status.changes()
    );

    let standing = repo.worktrees();
    assert_eq!(standing.len(), 2);
    assert!(standing[0].is_main() && standing[0].is_here());
    assert_eq!(standing[1].path(), made.path());
    assert!(!standing[1].is_main() && !standing[1].is_here());

    let branches = repo.branches(&status, &standing);
    assert!(
        branches
            .iter()
            .any(|branch| branch.name() == "feature/one" && branch.is_checked_out_elsewhere())
    );
}

#[test]
fn missing_worktree_is_pruned() {
    let Some((folder, repo)) = repository() else {
        return;
    };
    let root = folder.path();

    let made = repo
        .add_worktree("apart", Some("main"))
        .expect("a worktree to delete");
    std::fs::remove_dir_all(made.path()).expect("the worktree folder is deleted");

    let standing = repo.worktrees();
    assert_eq!(
        standing.len(),
        1,
        "a worktree that is not on disk is not one of them: {standing:?}"
    );
    assert!(standing[0].is_here());

    let status = repo.status().expect("git says what changed");
    assert!(
        repo.branches(&status, &standing)
            .iter()
            .any(|branch| branch.name() == "apart" && !branch.is_checked_out_elsewhere()),
        "removed worktree's branch must be free"
    );

    let again = repo
        .add_worktree("apart", None)
        .expect("the branch can be checked out again");
    assert_eq!(again.path(), root.join(".rikka/worktrees/apart"));
}
