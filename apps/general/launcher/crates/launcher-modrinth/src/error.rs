#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] rikka_net::Error),
    #[error("failed to encode a Modrinth query: {0}")]
    Query(#[from] serde_json::Error),
    #[error("Modrinth has no project {0}")]
    NoProject(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
