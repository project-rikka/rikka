#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("no signed-in model account")]
    Unconfigured,

    #[error("{model} does not accept images")]
    Blind { model: String },

    #[error("text has {length} characters, limit is {most}")]
    TooMuch { length: usize, most: usize },

    #[error("failed to render prompt: {0}")]
    Prompt(#[from] rikka_prompt::Error),

    #[error("failed to prepare image: {0}")]
    Picture(String),

    #[error("{0}")]
    Refused(String),

    #[error("model response is not a translation: {0}")]
    Malformed(String),
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
