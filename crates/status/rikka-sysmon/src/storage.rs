use std::path::{Path, PathBuf};

use rikka_mounts::Mount;

const FILESYSTEMS: [&str; 16] = [
    "ext2", "ext3", "ext4", "xfs", "btrfs", "bcachefs", "f2fs", "zfs", "jfs", "reiserfs", "vfat",
    "exfat", "ntfs", "ntfs3", "fuseblk", "hfsplus",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    pub mount: PathBuf,
    pub device: String,
    pub filesystem: String,
    pub read_only: bool,
    pub total: u64,
    pub free: u64,
    pub available: u64,
}

impl Volume {
    #[must_use]
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.free)
    }

    #[must_use]
    pub fn fraction(&self) -> f32 {
        let usable = self.used() + self.available;
        if usable == 0 {
            return 0.0;
        }
        (self.used() as f32 / usable as f32).clamp(0.0, 1.0)
    }
}

pub(crate) fn volumes() -> Vec<Volume> {
    storage(rikka_mounts::mounts())
        .into_iter()
        .filter_map(|mount| {
            let (total, free, available) = capacity(&mount.at)?;
            (total > 0).then_some(Volume {
                mount: mount.at,
                device: mount.source,
                filesystem: mount.filesystem,
                read_only: mount.read_only,
                total,
                free,
                available,
            })
        })
        .collect()
}

fn storage(mounts: Vec<Mount>) -> Vec<Mount> {
    let mut found: Vec<Mount> = Vec::new();
    for mount in mounts {
        if !FILESYSTEMS.contains(&mount.filesystem.as_str()) {
            continue;
        }
        match found.iter_mut().find(|known| known.source == mount.source) {
            Some(known) if mount.at.as_os_str().len() < known.at.as_os_str().len() => {
                *known = mount;
            }
            Some(_) => {}
            None => found.push(mount),
        }
    }
    found.sort_by(|left, right| left.at.cmp(&right.at));
    found
}

fn capacity(mount: &Path) -> Option<(u64, u64, u64)> {
    let stats = rustix::fs::statvfs(mount).ok()?;
    let block = stats.f_frsize;
    Some((
        stats.f_blocks * block,
        stats.f_bfree * block,
        stats.f_bavail * block,
    ))
}
