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

    #[error(transparent)]
    Gpu(#[from] rikka_gpu::Error),
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

pub fn av_err(context: &'static str, code: c_int) -> Error {
    rikka_ffmpeg::av_err(context, code).into()
}

pub fn check(context: &'static str, code: c_int) -> Result<c_int> {
    Ok(rikka_ffmpeg::check(context, code)?)
}
