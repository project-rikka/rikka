use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

const REMOTE_FILESYSTEMS: [u64; 10] = [
    0x6969,
    0x517B,
    0xFF53_4D42,
    0xFE53_4D42,
    0x6573_5546,
    0x0102_1997,
    0x00C3_6400,
    0x5346_414F,
    0x7375_7245,
    0x564C,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RootState {
    pub(crate) online: bool,
    pub(crate) remote: bool,
    pub(crate) device: Option<i64>,
}

impl RootState {
    pub(crate) fn inspect(path: &Path, known_device: Option<i64>) -> RootState {
        let offline = RootState {
            online: false,
            remote: false,
            device: known_device,
        };
        let Ok(metadata) = fs::metadata(path) else {
            return offline;
        };
        if !metadata.is_dir() {
            return offline;
        }
        let device = i64::from_ne_bytes(metadata.dev().to_ne_bytes());
        let empty = fs::read_dir(path).map_or(true, |mut entries| entries.next().is_none());
        if empty && known_device.is_some_and(|known| known != device) {
            return offline;
        }
        let remote = rustix::fs::statfs(path).is_ok_and(|stats| {
            let kind = stats.f_type as u64 & 0xFFFF_FFFF;
            REMOTE_FILESYSTEMS.contains(&kind)
        });
        RootState {
            online: true,
            remote,
            device: Some(device),
        }
    }
}

pub(crate) fn outermost(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut minimal: Vec<PathBuf> = Vec::new();
    for root in roots {
        let covered = roots
            .iter()
            .any(|other| other != root && root.starts_with(other));
        if !covered && !minimal.contains(root) {
            minimal.push(root.clone());
        }
    }
    minimal
}
