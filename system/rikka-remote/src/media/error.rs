#[derive(Debug, thiserror::Error)]
pub(crate) enum MediaError {
    #[error("cannot start {thread}: {error}")]
    Thread {
        thread: &'static str,
        error: std::io::Error,
    },

    #[error("the picture stopped before it began")]
    PictureGone,

    #[error("cannot capture the screen: {0}")]
    Capture(rikka_capture::Error),

    #[error("cannot capture {display}: {error}")]
    Display {
        display: String,
        error: rikka_capture::Error,
    },

    #[error("cannot follow the cursor: {0}")]
    Cursor(rikka_capture::Error),

    #[error("the compositor is gone: {0}")]
    Compositor(rikka_capture::Error),

    #[error("cannot allocate a capture buffer: {0}")]
    Buffer(rikka_capture::Error),

    #[error("no GPU to encode on: {0}")]
    Gpu(rikka_gpu::Error),

    #[error("no codec both ends support")]
    NoCodec,

    #[error("the video encoder failed: {0}")]
    Encoder(#[from] rikka_vulkan_video::Error),

    #[error("cannot send the desktop's sound: {0}")]
    Sound(rikka_audio::Error),

    #[error("cannot reach the clipboard: {0}")]
    Clipboard(rikka_clipboard::Error),

    #[error("cannot control the desktop: {0}")]
    Input(rikka_virtual_input::Error),

    #[error("{thread} stopped before it began")]
    Exited { thread: &'static str },
}

impl MediaError {
    pub(crate) fn worker(
        thread: &'static str,
        error: rikka_worker::Error<MediaError>,
    ) -> MediaError {
        match error {
            rikka_worker::Error::Spawn(error) => MediaError::Thread { thread, error },
            rikka_worker::Error::Startup(error) => error,
            rikka_worker::Error::Exited => MediaError::Exited { thread },
        }
    }
}
