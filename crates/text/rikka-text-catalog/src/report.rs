use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum LoadProblem {
    #[error("fontconfig file not found: {0}")]
    MissingConfig(PathBuf),
    #[error("fontconfig include not found: {0}")]
    MissingInclude(PathBuf),
    #[error("cannot read fontconfig file {path}: {source}")]
    UnreadableConfig { path: PathBuf, source: io::Error },
    #[error("fontconfig file {0} has no <fontconfig> root element")]
    MalformedConfig(PathBuf),
    #[error("cannot read font file {path}: {source}")]
    UnreadableFont { path: PathBuf, source: io::Error },
    #[error("no usable face in font file {0}")]
    UnusableFont(PathBuf),
    #[error("cannot write font index {path}: {source}")]
    IndexWrite { path: PathBuf, source: io::Error },
    #[error("no usable face in embedded font {0}")]
    UnusableEmbeddedFont(&'static str),
    #[error("cannot list the system fonts: {0}")]
    SystemFonts(io::Error),
}

#[derive(Debug, Default)]
pub struct LoadReport {
    pub problems: Vec<LoadProblem>,
    pub scanned_files: usize,
    pub indexed_files: usize,
}
