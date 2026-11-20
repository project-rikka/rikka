use std::path::Path;

use crate::error::Result;
use crate::git::{self, git};

pub(crate) fn matching(root: &Path, pattern: &str) -> Result<String> {
    git(root)
        .args(["config", "--null", "--get-regexp", pattern])
        .differs(true)
        .read()
}

pub(crate) fn entries(output: &str) -> impl Iterator<Item = (&str, &str)> {
    git::fields(output).map(|entry| entry.split_once('\n').unwrap_or((entry, "true")))
}
