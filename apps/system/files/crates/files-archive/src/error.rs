use files_core::Location;
use files_vfs::Error;

#[derive(Clone, Debug)]
pub(crate) enum ArchiveError {
    Locked { wrong: bool },
    Exists,
    Cancelled,
    Other(String),
}

impl ArchiveError {
    pub(crate) fn from_archive(error: &rikka_archive::Error) -> Self {
        use rikka_archive::Error as LibraryError;

        match error {
            LibraryError::Cancelled => Self::Cancelled,
            LibraryError::PasswordRequired => Self::Locked { wrong: false },
            LibraryError::WrongPassword => Self::Locked { wrong: true },
            other => Self::Other(other.to_string()),
        }
    }

    pub(crate) fn at(self, at: &Location) -> Error {
        match self {
            Self::Locked { wrong } => Error::Locked {
                at: at.clone(),
                wrong,
            },
            Self::Exists => Error::Exists { at: at.clone() },
            Self::Cancelled => Error::Cancelled,
            Self::Other(message) => Error::Other {
                at: at.clone(),
                message,
            },
        }
    }
}
