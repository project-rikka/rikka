use std::path::Path;

use crate::address::Address;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Folder,
    File,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Anything,
    Create,
    Rename,
    Trash,
    Write,
    Move,
    Restore,
}

#[derive(Clone, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error<L: Address> {
    #[error("{}: not found", at.name())]
    NotFound { at: L },
    #[error("{}: permission denied", at.name())]
    Denied { at: L },
    #[error("{}: already exists", at.name())]
    Exists { at: L },
    #[error("{}: not a {}", at.name(), expected.as_str())]
    WrongKind { at: L, expected: Kind },
    #[error("{}: directory not empty", at.name())]
    NotEmpty { at: L },
    #[error("{}: no space left on device", at.name())]
    NoSpace { at: L },
    #[error("{}: cannot move into itself", at.name())]
    IntoItself { at: L },
    #[error("{name}: invalid file name")]
    BadName { name: String },
    #[error("{}: no backend for this place", at.name())]
    NoBackend { at: L },
    #[error("{}: cannot {}", at.name(), action.as_str())]
    Unsupported { at: L, action: Action },
    #[error("{}: {}", at.name(), if *wrong { "wrong password" } else { "password required" })]
    Locked { at: L, wrong: bool },
    #[error("cancelled")]
    Cancelled,
    #[error("{}: {message}", at.name())]
    Other { at: L, message: String },
}

impl<L: Address> Error<L> {
    #[must_use]
    pub fn from_io(error: &std::io::Error, at: &L) -> Self {
        use std::io::ErrorKind;
        let at = at.clone();
        match error.kind() {
            ErrorKind::NotFound => Self::NotFound { at },
            ErrorKind::PermissionDenied => Self::Denied { at },
            ErrorKind::AlreadyExists => Self::Exists { at },
            ErrorKind::DirectoryNotEmpty => Self::NotEmpty { at },
            ErrorKind::NotADirectory => Self::WrongKind {
                at,
                expected: Kind::Folder,
            },
            ErrorKind::IsADirectory => Self::WrongKind {
                at,
                expected: Kind::File,
            },
            ErrorKind::Unsupported => Self::Unsupported {
                at,
                action: Action::Write,
            },
            ErrorKind::StorageFull => Self::NoSpace { at },
            _ => Self::Other {
                at,
                message: error.to_string(),
            },
        }
    }

    #[must_use]
    pub fn map<M: Address>(self, place: impl FnOnce(L) -> M) -> Error<M> {
        match self {
            Self::NotFound { at } => Error::NotFound { at: place(at) },
            Self::Denied { at } => Error::Denied { at: place(at) },
            Self::Exists { at } => Error::Exists { at: place(at) },
            Self::WrongKind { at, expected } => Error::WrongKind {
                at: place(at),
                expected,
            },
            Self::NotEmpty { at } => Error::NotEmpty { at: place(at) },
            Self::NoSpace { at } => Error::NoSpace { at: place(at) },
            Self::IntoItself { at } => Error::IntoItself { at: place(at) },
            Self::BadName { name } => Error::BadName { name },
            Self::NoBackend { at } => Error::NoBackend { at: place(at) },
            Self::Unsupported { at, action } => Error::Unsupported {
                at: place(at),
                action,
            },
            Self::Locked { at, wrong } => Error::Locked {
                at: place(at),
                wrong,
            },
            Self::Cancelled => Error::Cancelled,
            Self::Other { at, message } => Error::Other {
                at: place(at),
                message,
            },
        }
    }
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::File => "file",
        }
    }
}

impl Action {
    fn as_str(self) -> &'static str {
        match self {
            Self::Anything => "modify",
            Self::Create => "create",
            Self::Rename => "rename",
            Self::Trash => "trash",
            Self::Write => "write",
            Self::Move => "move",
            Self::Restore => "restore",
        }
    }
}

pub type Result<T, L> = std::result::Result<T, Error<L>>;

pub(crate) fn check_name<L: Address>(name: &str) -> Result<(), L> {
    let bad =
        name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\0');
    if bad {
        return Err(Error::BadName {
            name: name.to_owned(),
        });
    }
    Ok(())
}

pub(crate) fn name_of<L: Address>(path: &Path) -> Result<&str, L> {
    path.file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::BadName {
            name: path.display().to_string(),
        })
}
