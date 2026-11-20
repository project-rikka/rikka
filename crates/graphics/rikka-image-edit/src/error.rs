#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("GPU error: {0}")]
    Gpu(#[from] rikka_gpu::Error),

    #[error(transparent)]
    Picture(#[from] rikka_image::Error),

    #[error("invalid colour profile: {0}")]
    Profile(String),

    #[error("{width}×{height} picture exceeds device limits")]
    TooLarge { width: u32, height: u32 },

    #[error("{0}")]
    Invalid(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
