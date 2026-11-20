use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    path: PathBuf,
    canonical_path: PathBuf,
    branch: Option<String>,
    here: bool,
    main: bool,
}

impl Worktree {
    pub(crate) fn new(path: PathBuf, branch: Option<String>, here: &Path, main: bool) -> Worktree {
        let canonical_path = canonical_path(&path);
        Worktree {
            here: canonical_path == here,
            path,
            canonical_path,
            branch,
            main,
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }

    #[must_use]
    pub fn branch(&self) -> Option<&str> {
        self.branch.as_deref()
    }

    #[must_use]
    pub fn is_here(&self) -> bool {
        self.here
    }

    #[must_use]
    pub fn is_main(&self) -> bool {
        self.main
    }
}

pub(crate) fn canonical_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_owned())
}
