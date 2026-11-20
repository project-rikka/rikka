use std::convert::Infallible;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot start the thumbnail decoders: {0}")]
    Spawn(#[from] rikka_worker::Error<Infallible>),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
