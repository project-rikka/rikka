use ade_git::{Error, State};

use crate::support::{Linked, commit, git, linked};

const FAR: [&str; 2] = ["far.txt", "far\n"];

const NEAR: [&str; 2] = ["near.txt", "near\n"];

fn diverge(
    linked: &Linked,
    [far, far_text]: [&str; 2],
    [near, near_text]: [&str; 2],
) -> Option<()> {
    let other = linked.other();
    commit(&other, far, far_text, "far")?;
    git(&other, &["push", "--quiet"])?;
    commit(linked.root(), near, near_text, "near")
}

#[test]
fn fetch_counts_commits_behind() {
    let Some(linked) = linked() else {
        return;
    };
    let other = linked.other();
    let [far, far_text] = FAR;
    commit(&other, far, far_text, "far").expect("a commit elsewhere");
    git(&other, &["push", "--quiet"]).expect("it reaches the remote");

    let before = linked.repo.state().expect("git says where it is");
    assert_eq!(before.remote.as_deref(), Some("origin"));
    assert_eq!(before.fetched, None);
    assert_eq!(before.status.behind(), 0);

    linked.repo.fetch("origin", |_| {}).expect("it fetches");

    let after = linked.repo.state().expect("git says where it is now");
    assert_eq!(after.status.behind(), 1);
    assert!(after.fetched.is_some());
}

#[test]
fn pull_merges_diverged_branch_without_settings() {
    let Some(linked) = linked() else {
        return;
    };
    diverge(&linked, FAR, NEAR).expect("both sides commit");
    linked.repo.fetch("origin", |_| {}).expect("it fetches");
    let status = linked.repo.status().expect("git says where it is");
    assert_eq!((status.ahead(), status.behind()), (1, 1));

    linked.repo.pull(&status, |_| {}).expect("it merges");

    let after = linked.repo.status().expect("git says where it is now");
    assert_eq!((after.ahead(), after.behind()), (2, 0));
    assert!(linked.root().join(FAR[0]).is_file());
    assert_eq!(linked.repo.history().expect("commits").len(), 4);
}

#[test]
fn pull_follows_rebase_setting() {
    let Some(linked) = linked() else {
        return;
    };
    git(linked.root(), &["config", "pull.rebase", "true"]).expect("a pull setting");
    diverge(&linked, FAR, NEAR).expect("both sides commit");
    let status = linked.repo.status().expect("git says where it is");

    linked.repo.pull(&status, |_| {}).expect("it rebases");

    let after = linked.repo.status().expect("git says where it is now");
    assert_eq!((after.ahead(), after.behind()), (1, 0));
    assert_eq!(linked.repo.history().expect("commits").len(), 3);
}

#[test]
fn pull_stops_on_conflicts() {
    let Some(linked) = linked() else {
        return;
    };
    diverge(
        &linked,
        ["src/main.rs", "fn main() {\n    let one = 2;\n}\n"],
        ["src/main.rs", "fn main() {\n    let one = 3;\n}\n"],
    )
    .expect("both sides change one line");
    let status = linked.repo.status().expect("git says where it is");

    let pulled = linked.repo.pull(&status, |_| {});

    assert!(matches!(pulled, Err(Error::Conflicts)), "{pulled:?}");
    let after = linked.repo.status().expect("git says where it stopped");
    assert!(
        after
            .changes()
            .iter()
            .any(|change| change.state() == State::Conflicted)
    );
}

#[test]
fn push_is_rejected_when_remote_moved() {
    let Some(linked) = linked() else {
        return;
    };
    diverge(&linked, FAR, NEAR).expect("both sides commit");
    let status = linked.repo.status().expect("git says where it is");

    let pushed = linked.repo.push(&status, |_| {});

    assert!(matches!(pushed, Err(Error::Rejected)), "{pushed:?}");
}

#[test]
fn publish_tracks_branch_and_reports_rising_progress() {
    let Some(linked) = linked() else {
        return;
    };
    git(linked.root(), &["switch", "--quiet", "-c", "feature"]).expect("a new branch");
    commit(linked.root(), "feature.txt", "feature\n", "feature").expect("a commit on it");
    let status = linked.repo.status().expect("git says where it is");
    assert_eq!(status.upstream(), None);

    let mut fractions = Vec::new();
    linked
        .repo
        .push(&status, |progress| fractions.push(progress.fraction()))
        .expect("it publishes");

    let after = linked.repo.status().expect("git says where it is now");
    assert_eq!(after.upstream(), Some("origin/feature"));
    assert!(!fractions.is_empty());
    assert!(fractions.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(
        fractions
            .iter()
            .all(|fraction| (0.0..=1.0).contains(fraction))
    );
}
