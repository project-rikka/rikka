use std::path::{Path, PathBuf};

use rikka_format::natural_cmp;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VolumeId(pub(crate) String);

impl VolumeId {
    #[must_use]
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum VolumeKind {
    Internal,
    Removable,
    Optical,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Actions {
    pub mount: bool,
    pub unmount: bool,
    pub eject: bool,
    pub unlock: bool,
    pub lock: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    pub id: VolumeId,
    pub device: PathBuf,
    pub label: Option<String>,
    pub drive: Option<String>,
    pub size: u64,
    pub kind: VolumeKind,
    pub mount: Option<PathBuf>,
    pub locked: bool,
    pub is_root: bool,
    pub actions: Actions,
}

const RESERVED: &[&str] = &["/boot", "/efi"];

pub(crate) fn is_reserved(at: &Path) -> bool {
    RESERVED.iter().any(|reserved| at.starts_with(reserved))
}

pub(crate) fn ordered(volumes: &mut Vec<Volume>) {
    volumes.sort_by(|one, other| {
        one.kind.cmp(&other.kind).then_with(|| {
            natural_cmp(
                &one.device.to_string_lossy(),
                &other.device.to_string_lossy(),
            )
        })
    });
    volumes.dedup_by(|one, other| one.device == other.device);
}
