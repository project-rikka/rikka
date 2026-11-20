#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("invalid working directory {path}: {reason}")]
    Rootless { path: String, reason: String },

    #[error("failed to load tool prompts: {0}")]
    Document(#[from] rikka_prompt::Error),

    #[error(transparent)]
    Unknown(#[from] rikka_tool_docs::Error),

    #[error("failed to declare tool \"{name}\": {reason}")]
    Undeclarable { name: String, reason: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
