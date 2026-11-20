use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rikka_mounts::Mount;

use crate::volume::{self, Actions, Volume, VolumeId, VolumeKind};

pub(crate) fn mounted() -> Vec<Volume> {
    let labels = labels();
    let mut volumes: Vec<Volume> = rikka_mounts::mounts()
        .into_iter()
        .filter(Mount::is_storage)
        .filter(worth_showing)
        .map(|mount| {
            let device = PathBuf::from(&mount.source);
            let node = device_name(&device);
            Volume {
                label: labels.get(&device).cloned(),
                drive: None,
                size: node.as_deref().map(size_of).unwrap_or(0),
                kind: node.as_deref().map_or(VolumeKind::Internal, kind_of),
                is_root: mount.at == Path::new("/"),
                locked: false,
                actions: Actions::default(),
                id: VolumeId(mount.at.to_string_lossy().into_owned()),
                mount: Some(mount.at),
                device,
            }
        })
        .collect();
    volume::ordered(&mut volumes);
    volumes
}

fn worth_showing(mount: &Mount) -> bool {
    Path::new(&mount.source).starts_with("/dev") && !volume::is_reserved(&mount.at)
}

fn labels() -> HashMap<PathBuf, String> {
    let Ok(entries) = std::fs::read_dir("/dev/disk/by-label") else {
        return HashMap::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let device = std::fs::canonicalize(entry.path()).ok()?;
            let label = entry.file_name().to_string_lossy().into_owned();
            Some((device, unescape(&label)))
        })
        .collect()
}

fn unescape(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut rest = label;
    while let Some(index) = rest.find("\\x") {
        out.push_str(&rest[..index]);
        let hex = rest.get(index + 2..index + 4);
        match hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
            Some(byte) => {
                out.push(byte as char);
                rest = &rest[index + 4..];
            }
            None => {
                out.push_str("\\x");
                rest = &rest[index + 2..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn device_name(source: &Path) -> Option<String> {
    let name = source.strip_prefix("/dev").ok()?;
    Some(name.to_string_lossy().replace('/', "!"))
}

fn kind_of(device: &str) -> VolumeKind {
    let disk = disk_of(device);
    if device.starts_with("sr") {
        return VolumeKind::Optical;
    }
    if read_flag(&format!("/sys/block/{disk}/removable")) {
        return VolumeKind::Removable;
    }
    VolumeKind::Internal
}

fn read_flag(path: &str) -> bool {
    std::fs::read_to_string(path).is_ok_and(|value| value.trim() == "1")
}

fn size_of(device: &str) -> u64 {
    const SECTOR: u64 = 512;
    let disk = disk_of(device);
    let sectors = std::fs::read_to_string(format!("/sys/block/{disk}/{device}/size"))
        .or_else(|_| std::fs::read_to_string(format!("/sys/block/{device}/size")))
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(0);
    sectors * SECTOR
}

fn disk_of(device: &str) -> String {
    if let Some(index) = device.rfind('p')
        && device[index + 1..].chars().all(|c| c.is_ascii_digit())
        && !device[index + 1..].is_empty()
        && device[..index].chars().any(|c| c.is_ascii_digit())
    {
        return device[..index].to_owned();
    }
    device
        .trim_end_matches(|c: char| c.is_ascii_digit())
        .to_owned()
}
