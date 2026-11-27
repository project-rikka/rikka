#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("error: not_found {0}")]
    NotFound(String),

    #[error("error: outside_workspace {0}")]
    OutsideWorkspace(String),

    #[error("error: not_text {0}")]
    NotText(String),

    #[error("error: too_big {path} holds {bytes} bytes")]
    TooBig { path: String, bytes: u64 },

    #[error("error: concurrent_modification {0} changed during the write")]
    ConcurrentModification(String),

    #[error("error: stale_anchor {path}: nothing was written.\n{report}")]
    StaleAnchor { path: String, report: String },

    #[error("error: reversed_range {path}: nothing was written.\n{report}")]
    ReversedRange { path: String, report: String },

    #[error("error: overlap {path}: nothing was written.\n{report}")]
    Overlap { path: String, report: String },

    #[error("error: listing_prefix {path}: nothing was written.\n{report}")]
    ListingPrefix { path: String, report: String },

    #[error("error: unchanged {path}: nothing was written.\n{report}")]
    Unchanged { path: String, report: String },

    #[error("error: echoed_line {path}: nothing was written.\n{report}")]
    EchoedLine { path: String, report: String },

    #[error("error: broken_syntax {path}: nothing was written.\n{report}")]
    BrokenSyntax { path: String, report: String },

    #[error("error: io {path} {reason}")]
    Io { path: String, reason: String },

    #[error("error: history_unavailable cannot record {path} for undo: {reason}")]
    HistoryWrite { path: String, reason: String },

    #[error("error: empty no edits given")]
    NoEdits,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
