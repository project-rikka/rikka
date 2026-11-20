use rikka_llm::Trouble;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("model error: {0}")]
    Model(rikka_llm::Error),

    #[error("tool \"{0}\" is declared twice")]
    DuplicateTool(String),

    #[error("subagent tool requires a name and description")]
    Anonymous,

    #[error("run cancelled")]
    Cancelled,

    #[error("agent has no compaction configured")]
    Unfoldable,

    #[error("nothing to compact")]
    NothingToFold,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<rikka_llm::Error> for Error {
    fn from(error: rikka_llm::Error) -> Error {
        match error {
            rikka_llm::Error::Cancelled => Error::Cancelled,
            other => Error::Model(other),
        }
    }
}

impl From<&Error> for Trouble {
    fn from(error: &Error) -> Trouble {
        let trouble = Trouble::new(error.to_string());
        match error {
            Error::Model(model) => trouble.caused_by(model),
            _ => trouble,
        }
    }
}
