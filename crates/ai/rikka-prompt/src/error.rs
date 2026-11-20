#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("{document}.md: {reason}")]
    Malformed { document: String, reason: String },

    #[error("{document}.md states no {kind} named \"{name}\"")]
    Unstated {
        document: String,
        kind: &'static str,
        name: String,
    },

    #[error("{document}.md references unsupplied {{{{{name}}}}}")]
    Unresolved { document: String, name: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
