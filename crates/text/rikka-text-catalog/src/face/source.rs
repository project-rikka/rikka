use std::path::Path;
use std::sync::Arc;

use crate::discovery::FileStamp;

#[derive(Clone, Debug)]
pub(crate) enum FaceSource {
    File { path: Arc<Path>, stamp: FileStamp },
    Embedded(&'static [u8]),
}

impl FaceSource {
    pub(crate) fn path(&self) -> Option<&Path> {
        match self {
            FaceSource::File { path, .. } => Some(path),
            FaceSource::Embedded(_) => None,
        }
    }
}
