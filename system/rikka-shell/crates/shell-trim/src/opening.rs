use std::path::{Path, PathBuf};

use rikka_seam::Request;
use serde::{Deserialize, Serialize};
use shell_notifications::ActionRequest;

use crate::service::{CAPABILITY, EDIT};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Opening {
    pub(crate) file: PathBuf,
    #[serde(default)]
    pub(crate) draft: bool,
}

impl Opening {
    #[must_use]
    pub(crate) fn recording(file: PathBuf) -> Opening {
        Opening { file, draft: false }
    }

    #[must_use]
    pub(crate) fn draft(file: PathBuf) -> Opening {
        Opening { file, draft: true }
    }
}

#[must_use]
pub fn action_request(file: &Path) -> ActionRequest {
    ActionRequest::new(CAPABILITY, EDIT).encode(&Opening::recording(file.to_path_buf()))
}

#[must_use]
pub(crate) fn opening(opening: &Opening) -> Request {
    Request::new(CAPABILITY, EDIT).encode(opening)
}
