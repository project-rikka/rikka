#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("malformed message from rikka-nixctl: {0}")]
    Malformed(String),
}
