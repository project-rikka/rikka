use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Subject {
    File(PathBuf),
    Commit(String),
}
