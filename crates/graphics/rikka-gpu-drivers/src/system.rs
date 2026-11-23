use std::path::PathBuf;

use crate::extend_unique;
use crate::kernel::{Kernel, machine, mesa_manifest_prefix};
use crate::manifest::{
    EGL_FILENAMES_ENV, egl_vendors, named_like, serving, vulkan_chosen_elsewhere,
    vulkan_manifest_files,
};

pub const VULKAN_SELECT_ENV: &str = "VK_LOADER_DRIVERS_SELECT";

pub unsafe fn narrow_system() {
    let machine = machine();
    if let Some(node) = machine.unknown.first() {
        tracing::debug!("no known graphics drivers for {}", node.display());
        return;
    }
    if machine.kernels.is_empty() {
        return;
    }
    unsafe {
        narrow_egl(&machine.kernels);
        narrow_vulkan(&machine.kernels);
    }
}

unsafe fn narrow_egl(kernels: &[&Kernel]) {
    if std::env::var_os(EGL_FILENAMES_ENV).is_some() {
        return;
    }
    let [kernel] = kernels else {
        return;
    };
    let vendors = egl_vendors();
    let Some(paths) = kernel
        .egl
        .iter()
        .map(|&vendor| serving(&vendors, vendor))
        .find(|paths| !paths.is_empty())
    else {
        tracing::debug!("no installed EGL vendor for kernel driver {}", kernel.name);
        return;
    };
    let Ok(filenames) = std::env::join_paths(paths) else {
        return;
    };
    tracing::info!("EGL vendors narrowed to {}", filenames.to_string_lossy());
    unsafe { std::env::set_var(EGL_FILENAMES_ENV, &filenames) };
}

unsafe fn narrow_vulkan(kernels: &[&Kernel]) {
    if vulkan_chosen_elsewhere() || std::env::var_os(VULKAN_SELECT_ENV).is_some() {
        return;
    }
    let Some(selection) = selection(kernels, &vulkan_manifest_files()) else {
        tracing::debug!("leaving Vulkan driver selection to the loader");
        return;
    };
    tracing::info!("Vulkan drivers narrowed to {selection}");
    unsafe { std::env::set_var(VULKAN_SELECT_ENV, selection) };
}

fn selection(kernels: &[&Kernel], installed: &[PathBuf]) -> Option<String> {
    let mut globs: Vec<String> = Vec::new();
    for kernel in kernels {
        let present: Vec<String> = kernel
            .mesa_vulkan
            .iter()
            .map(|name| mesa_manifest_prefix(name))
            .chain(kernel.vendor_vulkan.iter().map(|&prefix| prefix.to_owned()))
            .filter(|prefix| installed.iter().any(|path| named_like(path, prefix)))
            .map(|prefix| format!("{prefix}*"))
            .collect();
        if present.is_empty() {
            return None;
        }
        extend_unique(&mut globs, present);
    }
    Some(globs.join(","))
}
