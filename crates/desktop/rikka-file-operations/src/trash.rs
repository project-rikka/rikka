use std::path::PathBuf;

use crate::error::{Action, Error};

pub(crate) fn translated(error: rikka_trash::Error, action: Action) -> Error<PathBuf> {
    match error {
        rikka_trash::Error::NotFound(at) => Error::NotFound { at },
        rikka_trash::Error::Unsupported(at) => Error::Unsupported { at, action },
        rikka_trash::Error::Exists(at) => Error::Exists { at },
        rikka_trash::Error::Io { path, source } => Error::from_io(&source, &path),
    }
}
