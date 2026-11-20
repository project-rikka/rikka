#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Compute(#[from] rikka_gpu::Error),

    #[error("the NV12 surface does not have two planes")]
    Planes,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
