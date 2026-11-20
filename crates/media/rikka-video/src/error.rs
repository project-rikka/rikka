use core::ffi::c_int;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{context}: {message} (AVERROR {code})")]
    Av {
        context: &'static str,
        code: i32,
        message: String,
    },

    #[error("unsupported: {0}")]
    Unsupported(String),

    #[error("FFmpeg is unavailable: {0}")]
    Unavailable(String),

    #[error("playback worker terminated: {0}")]
    Worker(String),

    #[error("trim range keeps nothing")]
    EmptyCut,

    #[error("destination is the source file")]
    OverSource,

    #[error("source has no streams the destination can hold")]
    NothingToWrite,

    #[error("conversion cancelled")]
    Cancelled,

    #[error("the decoder produced a damaged picture")]
    Damaged,

    #[error(transparent)]
    Picture(#[from] rikka_image::Error),

    #[error(transparent)]
    Gpu(#[from] rikka_gpu::Error),

    #[cfg(unix)]
    #[error("Vulkan Video encoder failed: {0}")]
    VulkanVideo(#[from] rikka_vulkan_video::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<rikka_ffmpeg::Error> for Error {
    fn from(error: rikka_ffmpeg::Error) -> Self {
        match error {
            rikka_ffmpeg::Error::Av {
                context,
                code,
                message,
            } => Error::Av {
                context,
                code,
                message,
            },
            rikka_ffmpeg::Error::Unsupported(message) => Error::Unsupported(message),
            rikka_ffmpeg::Error::Unavailable(message) => Error::Unavailable(message),
        }
    }
}

impl From<rikka_video_core::Error> for Error {
    fn from(error: rikka_video_core::Error) -> Self {
        match error {
            rikka_video_core::Error::Av {
                context,
                code,
                message,
            } => Error::Av {
                context,
                code,
                message,
            },
            rikka_video_core::Error::Unsupported(message) => Error::Unsupported(message),
            rikka_video_core::Error::Unavailable(message) => Error::Unavailable(message),
            rikka_video_core::Error::Worker(message) => Error::Worker(message),
            rikka_video_core::Error::Gpu(error) => Error::Gpu(error),
        }
    }
}

#[cfg(unix)]
impl From<rikka_dmabuf_convert::Error> for Error {
    fn from(error: rikka_dmabuf_convert::Error) -> Self {
        match error {
            rikka_dmabuf_convert::Error::Compute(error) => Error::Gpu(error),
            rikka_dmabuf_convert::Error::Planes => Error::Unsupported(error.to_string()),
        }
    }
}

pub(crate) fn check(context: &'static str, code: c_int) -> Result<c_int> {
    Ok(rikka_ffmpeg::check(context, code)?)
}
