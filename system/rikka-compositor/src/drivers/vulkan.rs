use std::path::PathBuf;

use rikka_dirs::{config_dirs, data_dirs};

use super::RenderNode;

pub(crate) const SELECT_ENV: &str = "VK_LOADER_DRIVERS_SELECT";

const CHOSEN_ELSEWHERE: &[&str] = &[
    SELECT_ENV,
    "VK_DRIVER_FILES",
    "VK_ICD_FILENAMES",
    "VK_ADD_DRIVER_FILES",
];

const MANIFESTS: &str = "vulkan/icd.d";

const SYSTEM_CONFIG: &str = "/etc";

const NIXOS_DRIVERS: &str = "/run/opengl-driver/share";

const MANIFESTS_BY_KERNEL_DRIVER: &[(&str, &[&str])] = &[
    ("nvidia", &["nvidia_icd"]),
    ("nouveau", &["nouveau_icd"]),
    ("amdgpu", &["radeon_icd", "amd_icd"]),
    ("radeon", &["radeon_icd"]),
    ("i915", &["intel_icd", "intel_hasvk_icd"]),
    ("xe", &["intel_icd"]),
    ("virtio_gpu", &["virtio_icd"]),
];

pub(super) unsafe fn narrow(nodes: &[RenderNode]) {
    if CHOSEN_ELSEWHERE
        .iter()
        .any(|name| std::env::var_os(name).is_some())
    {
        return;
    }
    let Some(selection) = selection(nodes, &installed_manifests()) else {
        tracing::debug!("leaving Vulkan driver selection to the loader");
        return;
    };
    tracing::info!("Vulkan drivers narrowed to {selection}");
    unsafe { std::env::set_var(SELECT_ENV, selection) };
}

fn selection(nodes: &[RenderNode], installed: &[String]) -> Option<String> {
    if nodes.is_empty() {
        return None;
    }
    let mut globs: Vec<String> = Vec::new();
    for node in nodes {
        let (_, prefixes) = MANIFESTS_BY_KERNEL_DRIVER
            .iter()
            .find(|(driver, _)| *driver == node.driver)?;
        let present: Vec<&str> = prefixes
            .iter()
            .copied()
            .filter(|prefix| installed.iter().any(|name| name.starts_with(prefix)))
            .collect();
        if present.is_empty() {
            return None;
        }
        for prefix in present {
            let glob = format!("{prefix}*");
            if !globs.contains(&glob) {
                globs.push(glob);
            }
        }
    }
    Some(globs.join(","))
}

fn installed_manifests() -> Vec<String> {
    manifest_dirs()
        .into_iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.ends_with(".json"))
        .collect()
}

fn manifest_dirs() -> Vec<PathBuf> {
    config_dirs()
        .into_iter()
        .chain([PathBuf::from(SYSTEM_CONFIG)])
        .chain(data_dirs())
        .chain([PathBuf::from(NIXOS_DRIVERS)])
        .map(|root| root.join(MANIFESTS))
        .collect()
}
