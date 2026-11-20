use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("cannot run git: {0}")]
    Unavailable(String),

    #[error("{0} is not in a git repository")]
    Unversioned(PathBuf),

    #[error("{0}")]
    Refused(String),

    #[error("no remote to push to")]
    Remoteless,

    #[error("the branch has no upstream to pull from")]
    Upstreamless,

    #[error("the remote has commits that are not here yet; pull them first")]
    Rejected,

    #[error("the pulled commits conflict with local changes; resolve the conflicts, then commit")]
    Conflicts,

    #[error("the rebase stopped on conflicts; resolve them, then run git rebase --continue")]
    RebaseConflicts,

    #[error("no branch is checked out")]
    Headless,

    #[error("no files selected to commit")]
    Nothing,

    #[error("{0} is checked out in another worktree")]
    BranchCheckedOut(String),

    #[error("a commit needs a summary")]
    Unsummarised,

    #[error("a worktree needs a branch name")]
    Unnamed,

    #[error("cannot create {}: {reason}", path.display())]
    Create { path: PathBuf, reason: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
