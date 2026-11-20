use files_core::Location;
use files_vfs::Error;

#[derive(Debug)]
pub(crate) enum ConvertError {
    Cancelled,
    Io(std::io::Error),
    Other(String),
}

impl ConvertError {
    pub(crate) fn at(self, at: &Location) -> Error {
        match self {
            ConvertError::Cancelled => Error::Cancelled,
            ConvertError::Io(error) => Error::from_io(&error, at),
            ConvertError::Other(message) => Error::Other {
                at: at.clone(),
                message,
            },
        }
    }
}

impl From<std::io::Error> for ConvertError {
    fn from(error: std::io::Error) -> ConvertError {
        ConvertError::Io(error)
    }
}

impl From<rikka_image::Error> for ConvertError {
    fn from(error: rikka_image::Error) -> ConvertError {
        match error {
            rikka_image::Error::Io(io) => ConvertError::Io(io),
            other => ConvertError::Other(other.to_string()),
        }
    }
}

impl From<rikka_audio_decode::Error> for ConvertError {
    fn from(error: rikka_audio_decode::Error) -> ConvertError {
        match error {
            rikka_audio_decode::Error::Io(io) => ConvertError::Io(io),
            other => ConvertError::Other(other.to_string()),
        }
    }
}

impl From<rikka_video::Error> for ConvertError {
    fn from(error: rikka_video::Error) -> ConvertError {
        match error {
            rikka_video::Error::Cancelled => ConvertError::Cancelled,
            other => ConvertError::Other(other.to_string()),
        }
    }
}

impl From<rikka_model::Error> for ConvertError {
    fn from(error: rikka_model::Error) -> ConvertError {
        match error {
            rikka_model::Error::Cancelled => ConvertError::Cancelled,
            rikka_model::Error::Io(io) => ConvertError::Io(io),
            other => ConvertError::Other(other.to_string()),
        }
    }
}
