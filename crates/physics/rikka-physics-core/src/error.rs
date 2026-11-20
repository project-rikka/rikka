#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("worker pool needs at least one worker")]
    NoWorkers,
    #[error("failed to spawn worker thread {name}")]
    Spawn {
        name: String,
        #[source]
        source: std::io::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
