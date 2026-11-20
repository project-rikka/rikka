#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no audio device named {0}")]
    DeviceMissing(String),

    #[error("audio device failed: {0}")]
    Device(String),

    #[error("Opus failed: {0}")]
    Codec(String),

    #[error("failed to spawn thread `{name}`")]
    Thread {
        name: &'static str,
        #[source]
        source: std::io::Error,
    },

    #[error("thread `{name}` stopped before it began")]
    Exited { name: &'static str },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<rikka_opus::Error> for Error {
    fn from(error: rikka_opus::Error) -> Error {
        Error::Codec(error.to_string())
    }
}

impl From<rikka_audio::Error> for Error {
    fn from(error: rikka_audio::Error) -> Error {
        match error {
            rikka_audio::Error::DeviceNotFound(name) => Error::DeviceMissing(name),
            rikka_audio::Error::NoInputDevice => {
                Error::DeviceMissing("the default microphone".into())
            }
            rikka_audio::Error::NoOutputDevice => Error::DeviceMissing("the default output".into()),
            other => Error::Device(other.to_string()),
        }
    }
}

impl From<Error> for discord_core::Error {
    fn from(error: Error) -> discord_core::Error {
        discord_core::Error::voice(error)
    }
}
