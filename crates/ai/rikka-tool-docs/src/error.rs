#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no tool declared by document \"{0}\"")]
    Unknown(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
