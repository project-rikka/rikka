use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use animate_body::{Body, Faces};
use rikka_model::Model;
use rikka_types::PixelSize;

use crate::error::BoothError;

#[derive(Clone, Debug)]
pub enum Sitter {
    Model(PathBuf),
    Stage(PathBuf),
    Accessory(PathBuf),
    Motion {
        motion: PathBuf,
        performer: Performer,
    },
    Camera(PathBuf),
}

#[derive(Clone)]
pub enum Performer {
    Figure {
        key: u64,
        model: Arc<Model>,
        body: Option<Arc<Body>>,
        faces: Arc<Faces>,
    },
    Mannequin,
}

impl std::fmt::Debug for Performer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Figure { key, body, .. } => f
                .debug_struct("Figure")
                .field("key", key)
                .field("humanoid", &body.is_some())
                .finish_non_exhaustive(),
            Self::Mannequin => f.write_str("Mannequin"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Order {
    pub sitter: Sitter,
    pub size: PixelSize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Stamp {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}

impl Stamp {
    pub(crate) fn of(path: &Path) -> Result<Self, BoothError> {
        let metadata = std::fs::metadata(path).map_err(BoothError::Read)?;
        Ok(Self {
            path: path.to_path_buf(),
            size: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn size(&self) -> u64 {
        self.size
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ModelKey {
    File(Stamp),
    Figure(u64),
    Mannequin,
}

#[derive(Clone, Debug)]
pub(crate) enum Source {
    Model(Arc<Model>),
    File(Stamp),
}
