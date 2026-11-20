use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("unrecognized archive format")]
    UnknownFormat,

    #[error("archive is encrypted and requires a password")]
    PasswordRequired,

    #[error("incorrect password")]
    WrongPassword,

    #[error("archive is damaged: {0}")]
    Damaged(String),

    #[error("unsupported: {0}")]
    Unsupported(String),

    #[error("no entry named \"{0}\"")]
    EntryNotFound(String),

    #[error("entry index {index} out of range ({count} entries)")]
    IndexOutOfRange { index: usize, count: usize },

    #[error("{0} holds exactly one file")]
    NotAContainer(crate::format::Format),

    #[error("operation cancelled")]
    Cancelled,

    #[error("entry \"{0}\" escapes the extraction directory")]
    UnsafeEntryPath(String),

    #[error("file exists: {}", .0.display())]
    AlreadyExists(PathBuf),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<zip::result::ZipError> for Error {
    fn from(err: zip::result::ZipError) -> Self {
        use zip::result::ZipError;

        match err {
            ZipError::Io(io) => Error::Io(io),
            ZipError::InvalidPassword => Error::WrongPassword,
            ZipError::FileNotFound => Error::EntryNotFound(String::new()),
            ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED) => Error::PasswordRequired,
            ZipError::UnsupportedArchive(msg) => Error::Unsupported(msg.to_owned()),
            ZipError::CompressionMethodNotSupported(id) => {
                Error::Unsupported(format!("zip compression method {id}"))
            }
            ZipError::InvalidArchive(msg) => Error::Damaged(msg.into_owned()),
            other => Error::Damaged(other.to_string()),
        }
    }
}

impl From<sevenz_rust2::Error> for Error {
    fn from(err: sevenz_rust2::Error) -> Self {
        use sevenz_rust2::Error as SevenZError;

        match err {
            SevenZError::Io(io, _) | SevenZError::FileOpen(io, _) => Error::Io(io),
            SevenZError::PasswordRequired => Error::PasswordRequired,
            SevenZError::MaybeBadPassword(_) => Error::WrongPassword,
            SevenZError::FileNotFound => Error::EntryNotFound(String::new()),
            SevenZError::UnsupportedCompressionMethod(name) => {
                Error::Unsupported(format!("7z compression method {name}"))
            }
            SevenZError::Unsupported(msg) => Error::Unsupported(msg.into_owned()),
            SevenZError::ExternalUnsupported => {
                Error::Unsupported("7z archives with external headers".to_owned())
            }
            other => Error::Damaged(other.to_string()),
        }
    }
}
