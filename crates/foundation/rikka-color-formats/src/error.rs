#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("invalid hex colour `{0}`: expected 3 or 6 hex digits with optional `#`")]
    NotHex(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
