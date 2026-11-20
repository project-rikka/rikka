use std::io;
use std::path::PathBuf;

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("usage: rikka-public-suffix-compiler <public_suffix_list.dat> <rules.rs>")]
    Usage,
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("line {line}: rule {rule:?} lies outside the ICANN and private sections")]
    Unsectioned { line: usize, rule: String },
    #[error("line {line}: rule {rule:?} is not a domain name")]
    MalformedRule { line: usize, rule: String },
    #[error("the list has no rules")]
    Empty,
}
