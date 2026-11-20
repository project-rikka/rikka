use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use super::RenderNode;

const FILENAMES_ENV: &str = "__EGL_VENDOR_LIBRARY_FILENAMES";
const DIRS_ENV: &str = "__EGL_VENDOR_LIBRARY_DIRS";
const DEFAULT_DIRS: &[&str] = &[
    "/etc/glvnd/egl_vendor.d",
    "/run/opengl-driver/share/glvnd/egl_vendor.d",
    "/usr/share/glvnd/egl_vendor.d",
];

const MESA: &str = "libEGL_mesa";

const LIBRARIES_BY_KERNEL_DRIVER: &[(&str, &[&str])] = &[
    ("nvidia", &["libEGL_nvidia", MESA]),
    ("nouveau", &[MESA]),
    ("amdgpu", &[MESA]),
    ("radeon", &[MESA]),
    ("i915", &[MESA]),
    ("xe", &[MESA]),
    ("virtio_gpu", &[MESA]),
];

pub(super) unsafe fn narrow(nodes: &[RenderNode]) {
    if std::env::var_os(FILENAMES_ENV).is_some() {
        return;
    }
    let Some(driver) = sole_driver(nodes) else {
        return;
    };
    let Some(filenames) = vendor_filenames(driver, &search_dirs()) else {
        tracing::debug!("no installed EGL vendor for kernel driver {driver}");
        return;
    };
    tracing::info!("EGL vendors narrowed to {}", filenames.to_string_lossy());
    unsafe { std::env::set_var(FILENAMES_ENV, &filenames) };
}

fn sole_driver(nodes: &[RenderNode]) -> Option<&str> {
    let (first, rest) = nodes.split_first()?;
    rest.iter()
        .all(|node| node.driver == first.driver)
        .then_some(first.driver.as_str())
}

fn search_dirs() -> Vec<PathBuf> {
    match std::env::var_os(DIRS_ENV) {
        Some(dirs) => std::env::split_paths(&dirs).collect(),
        None => DEFAULT_DIRS.iter().map(PathBuf::from).collect(),
    }
}

fn vendor_filenames(driver: &str, dirs: &[PathBuf]) -> Option<OsString> {
    let libraries = LIBRARIES_BY_KERNEL_DRIVER
        .iter()
        .find_map(|(bound, libraries)| (*bound == driver).then_some(*libraries))?;
    let installed = installed_vendors(dirs);
    for library in libraries {
        let paths: Vec<&Path> = installed
            .iter()
            .filter(|(_, named)| serves(named, library))
            .map(|(path, _)| path.as_path())
            .collect();
        if !paths.is_empty() {
            return std::env::join_paths(paths).ok();
        }
    }
    None
}

fn installed_vendors(dirs: &[PathBuf]) -> Vec<(PathBuf, String)> {
    let mut vendors = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension() == Some(OsStr::new("json")))
            .collect();
        paths.sort();
        for path in paths {
            if let Some(library) = library_of(&path) {
                vendors.push((path, library));
            }
        }
    }
    vendors
}

fn library_of(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let library = json.get("ICD")?.get("library_path")?.as_str()?;
    Some(Path::new(library).file_name()?.to_str()?.to_owned())
}

fn serves(file_name: &str, library: &str) -> bool {
    file_name
        .strip_prefix(library)
        .is_some_and(|rest| rest.starts_with(".so"))
}
