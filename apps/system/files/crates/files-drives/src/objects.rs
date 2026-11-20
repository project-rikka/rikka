use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use rikka_bus::ManagedObjects;
use rikka_bus::Properties;

use crate::volume::{self, Actions, Volume, VolumeId, VolumeKind};

pub(crate) const BLOCK: &str = "org.freedesktop.UDisks2.Block";
pub(crate) const FILESYSTEM: &str = "org.freedesktop.UDisks2.Filesystem";
pub(crate) const ENCRYPTED: &str = "org.freedesktop.UDisks2.Encrypted";
const DRIVE: &str = "org.freedesktop.UDisks2.Drive";
const PARTITION: &str = "org.freedesktop.UDisks2.Partition";
const LOOP: &str = "org.freedesktop.UDisks2.Loop";

const RESERVED: &[&str] = &[
    "c12a7328-f81f-11d2-ba4b-00a0c93ec93b",
    "21686148-6449-6e6f-744e-656564454649",
    "e3c9e316-0b5c-4db8-817d-f92df00215ae",
    "de94bba4-06d1-4d40-a16a-bfd50179d6ac",
    "0xef",
    "0xee",
];

pub(crate) fn volumes(objects: &ManagedObjects) -> Vec<Volume> {
    let unlocked = cleartext_devices(objects);
    let mut volumes: Vec<Volume> = objects
        .implementing(BLOCK)
        .filter_map(|(path, block)| volume(objects, path, block, &unlocked))
        .collect();
    volume::ordered(&mut volumes);
    volumes
}

fn cleartext_devices(objects: &ManagedObjects) -> HashMap<String, String> {
    objects
        .implementing(BLOCK)
        .filter_map(|(path, block)| Some((block_backing(block)?, path.to_owned())))
        .collect()
}

fn volume(
    objects: &ManagedObjects,
    path: &str,
    block: &Properties,
    unlocked: &HashMap<String, String>,
) -> Option<Volume> {
    if block.boolean("HintIgnore").unwrap_or(false) {
        return None;
    }
    if let Some(backing) = objects.get(path, LOOP)
        && backing.uint64("SetupByUID").unwrap_or(0) != u64::from(current_uid())
    {
        return None;
    }
    let filesystem = objects.get(path, FILESYSTEM);
    let locked = objects.get(path, ENCRYPTED).is_some() && !unlocked.contains_key(path);
    match block.string_ref("IdUsage") {
        Some("filesystem") if filesystem.is_some() => {}
        Some("crypto") if locked => {}
        _ => return None,
    }
    if reserved(objects, path) {
        return None;
    }

    let mount = filesystem.and_then(mount_point).filter(|_| !locked);
    if mount.as_deref().is_some_and(volume::is_reserved) {
        return None;
    }

    let drive = drive_of(objects, block);
    let kind = kind_of(drive);
    let ejectable = drive.is_some_and(|drive| {
        drive.boolean("Ejectable").unwrap_or(false)
            || (drive.boolean("Removable").unwrap_or(false)
                && drive.boolean("CanPowerOff").unwrap_or(false))
    });
    let media = drive.is_none_or(|drive| {
        !drive.boolean("MediaRemovable").unwrap_or(false)
            || drive.boolean("MediaAvailable").unwrap_or(false)
    });
    let is_root = mount.as_deref() == Some(Path::new("/"));

    Some(Volume {
        id: VolumeId(path.to_owned()),
        device: device_node(block),
        label: block
            .string_ref("IdLabel")
            .filter(|label| !label.is_empty())
            .map(ToOwned::to_owned),
        drive: drive.and_then(drive_name),
        size: block.uint64("Size").unwrap_or(0),
        kind,
        is_root,
        actions: Actions {
            mount: !locked && mount.is_none() && filesystem.is_some() && media,
            unmount: mount.is_some() && !is_root,
            eject: ejectable,
            unlock: locked,
            lock: block_backing(block).is_some(),
        },
        locked,
        mount,
    })
}

pub(crate) fn drive_path(objects: &ManagedObjects, block: &str) -> Option<String> {
    objects
        .get(block, BLOCK)?
        .object_path("Drive")
        .map(ToOwned::to_owned)
}

pub(crate) fn is_ejectable(objects: &ManagedObjects, drive: &str) -> bool {
    objects
        .get(drive, DRIVE)
        .is_some_and(|drive| drive.boolean("Ejectable").unwrap_or(false))
}

pub(crate) fn backing_path(objects: &ManagedObjects, block: &str) -> Option<String> {
    block_backing(objects.get(block, BLOCK)?)
}

pub(crate) fn mounted_at(objects: &ManagedObjects, block: &str) -> Option<PathBuf> {
    mount_point(objects.get(block, FILESYSTEM)?)
}

fn block_backing(block: &Properties) -> Option<String> {
    block
        .object_path("CryptoBackingDevice")
        .map(ToOwned::to_owned)
}

fn reserved(objects: &ManagedObjects, path: &str) -> bool {
    objects.get(path, PARTITION).is_some_and(|partition| {
        partition
            .string_ref("Type")
            .is_some_and(|kind| RESERVED.contains(&kind.to_lowercase().as_str()))
    })
}

fn drive_of<'a>(objects: &'a ManagedObjects, block: &Properties) -> Option<&'a Properties> {
    objects.get(block.object_path("Drive")?, DRIVE)
}

fn kind_of(drive: Option<&Properties>) -> VolumeKind {
    let Some(drive) = drive else {
        return VolumeKind::Internal;
    };
    let optical_media = drive
        .strings("MediaCompatibility")
        .is_some_and(|media| media.iter().any(|media| media.starts_with("optical")));
    if drive.boolean("Optical").unwrap_or(false) || optical_media {
        return VolumeKind::Optical;
    }
    let bus = drive.string_ref("ConnectionBus").unwrap_or_default();
    if drive.boolean("Removable").unwrap_or(false) || matches!(bus, "usb" | "sdio" | "ieee1394") {
        return VolumeKind::Removable;
    }
    VolumeKind::Internal
}

fn drive_name(drive: &Properties) -> Option<String> {
    let name = [drive.string_ref("Vendor"), drive.string_ref("Model")]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<&str>>()
        .join(" ");
    (!name.is_empty()).then_some(name)
}

fn device_node(block: &Properties) -> PathBuf {
    ["PreferredDevice", "Device"]
        .into_iter()
        .find_map(|key| path_of(&block.value::<Vec<u8>>(key)?))
        .unwrap_or_default()
}

fn mount_point(filesystem: &Properties) -> Option<PathBuf> {
    filesystem
        .list::<Vec<u8>>("MountPoints")?
        .into_iter()
        .find_map(|point| path_of(&point))
}

fn path_of(bytes: &[u8]) -> Option<PathBuf> {
    let bytes = bytes.strip_suffix(b"\0").unwrap_or(bytes);
    (!bytes.is_empty()).then(|| PathBuf::from(OsStr::from_bytes(bytes)))
}

fn current_uid() -> u32 {
    rustix::process::getuid().as_raw()
}
