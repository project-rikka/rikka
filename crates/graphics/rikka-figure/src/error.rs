#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("font family not found: {family}")]
    UnknownFont { family: String },

    #[error("failed to lay out text: {0}")]
    Unlettered(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
