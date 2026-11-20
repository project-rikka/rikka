use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::user::{User, deduplicated};

const DEV: &str = "/dev";
const VIDEO4LINUX: &str = "/sys/class/video4linux";
const PROC: &str = "/proc";

pub(crate) fn cameras() -> Vec<PathBuf> {
    cameras_in(Path::new(VIDEO4LINUX), Path::new(DEV))
}

fn cameras_in(classes: &Path, dev: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(classes) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with("video"))
        .filter(|name| {
            std::fs::read_to_string(classes.join(name).join("index"))
                .map_or(true, |index| index.trim() == "0")
        })
        .map(|name| dev.join(name))
        .collect();
    found.sort();
    found
}

pub(crate) fn users(cameras: &[PathBuf]) -> Vec<User> {
    users_in(Path::new(PROC), cameras, std::process::id())
}

fn users_in(proc: &Path, cameras: &[PathBuf], myself: u32) -> Vec<User> {
    if cameras.is_empty() {
        return Vec::new();
    }
    let cameras: HashSet<&Path> = cameras.iter().map(PathBuf::as_path).collect();
    let Ok(processes) = std::fs::read_dir(proc) else {
        return Vec::new();
    };
    let found = processes
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| *pid != myself)
        .filter(|pid| holds(&proc.join(pid.to_string()), &cameras))
        .filter_map(|pid| user(&proc.join(pid.to_string()), pid))
        .collect();
    deduplicated(found)
}

fn holds(process: &Path, cameras: &HashSet<&Path>) -> bool {
    let Ok(descriptors) = std::fs::read_dir(process.join("fd")) else {
        return false;
    };
    descriptors.flatten().any(|descriptor| {
        std::fs::read_link(descriptor.path()).is_ok_and(|target| cameras.contains(target.as_path()))
    })
}

fn user(process: &Path, pid: u32) -> Option<User> {
    let comm = std::fs::read_to_string(process.join("comm")).ok()?;
    let comm = comm.trim();
    if comm.starts_with("pipewire") || comm == "wireplumber" {
        return None;
    }
    let name = std::fs::read_link(process.join("exe"))
        .ok()
        .and_then(|exe| Some(program(exe.file_name()?.to_str()?)))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| comm.to_owned());
    Some(User {
        name,
        app_id: None,
        pid: Some(pid),
        icon: None,
    })
}

fn program(file: &str) -> String {
    let file = file.strip_suffix("-wrapped").unwrap_or(file);
    file.strip_prefix('.').unwrap_or(file).to_owned()
}
