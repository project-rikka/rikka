use ash::vk;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("this GPU has no Vulkan Video encode queue")]
    NoVideoQueue,
    #[error("this GPU cannot encode {0}")]
    Unsupported(String),
    #[error("no suitable GPU memory type")]
    NoMemoryType,
    #[error("Vulkan call failed: {0}")]
    Vulkan(#[from] vk::Result),
    #[error("the GPU compute path failed: {0}")]
    Compute(#[from] rikka_gpu::Error),
    #[error("the encoder reported status {0}")]
    Status(i32),
    #[error("the encoded picture did not fit its {0}-byte buffer")]
    Overflow(u64),
    #[error("frame {0} is no longer held as a reference")]
    Evicted(u32),
}

impl From<rikka_dmabuf_convert::Error> for Error {
    fn from(error: rikka_dmabuf_convert::Error) -> Error {
        match error {
            rikka_dmabuf_convert::Error::Compute(error) => Error::Compute(error),
            rikka_dmabuf_convert::Error::Planes => Error::Unsupported("two-plane pictures".into()),
        }
    }
}
