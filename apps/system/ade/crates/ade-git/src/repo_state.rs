use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::branch::Branch;
use crate::status::Status;
use crate::worktree::Worktree;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RepoState {
    pub status: Status,
    pub folders: BTreeSet<PathBuf>,
    pub branches: Vec<Branch>,
    pub worktrees: Vec<Worktree>,
    pub remote: Option<String>,
    pub fetched: Option<SystemTime>,
}
