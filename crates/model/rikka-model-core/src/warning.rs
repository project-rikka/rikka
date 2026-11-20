#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Warning {
    #[error("missing resource \"{uri}\": {reason}")]
    MissingResource { uri: String, reason: String },

    #[error("{0}")]
    Unsupported(String),

    #[error("{0}")]
    Repaired(String),
}
