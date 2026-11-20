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

    #[error("error: overlap op{one} and op{two} cover the same lines")]
    Overlap { one: usize, two: usize },

    #[error("error: out_of_range line {line}, {path} has {count}")]
    OutOfRange {
        path: String,
        line: usize,
        count: usize,
    },

    #[error("error: backwards_range line {end} comes before line {start} in {path}")]
    ReversedRange {
        path: String,
        start: usize,
        end: usize,
    },

    #[error("error: echoed_line {path}: nothing was written.\n{report}")]
    EchoedLine { path: String, report: String },

    #[error("error: broken_syntax {path}: nothing was written.\n{report}")]
    BrokenSyntax { path: String, report: String },

    #[error("error: io {path} {reason}")]
    Io { path: String, reason: String },

    #[error("error: history_unavailable cannot record {path} for undo: {reason}")]
    HistoryWrite { path: String, reason: String },

    #[error("error: empty no operations given")]
    NoOps,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
