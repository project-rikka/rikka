#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(
        "\"{0}\" cannot name an app's tools: use 1 to 24 ASCII letters and digits, a letter first"
    )]
    Unnamable(String),

    #[error("process already offers {offered}'s tools and cannot offer {requested}'s")]
    Offered { offered: String, requested: String },

    #[error("cannot offer tool {tool}: {reason}")]
    Unofferable { tool: String, reason: &'static str },

    #[error("no desktop session to offer tools to")]
    Sessionless,

    #[error("assistant is unreachable: {0}")]
    Unreachable(String),

    #[error("failed to offer tools: {0}")]
    Unoffered(#[from] std::io::Error),

    #[error("failed to prepare the kit's prompts: {0}")]
    Unprepared(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
