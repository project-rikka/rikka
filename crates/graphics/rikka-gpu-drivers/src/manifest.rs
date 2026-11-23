use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use rikka_dirs::{config_dirs, data_dirs};

use crate::kernel::EglVendor;

pub(crate) const NIXOS_DRIVERS: &str = "/run/opengl-driver";

pub(crate) const VULKAN_FILES_ENV: &str = "VK_DRIVER_FILES";

pub(crate) const EGL_FILENAMES_ENV: &str = "__EGL_VENDOR_LIBRARY_FILENAMES";

pub(crate) const VULKAN_MANIFESTS: &str = "vulkan/icd.d";

const VULKAN_CHOSEN_ELSEWHERE: &[&str] =
    &[VULKAN_FILES_ENV, "VK_ICD_FILENAMES", "VK_ADD_DRIVER_FILES"];

const SYSTEM_CONFIG: &str = "/etc";

const EGL_DIRS_ENV: &str = "__EGL_VENDOR_LIBRARY_DIRS";

const EGL_DEFAULT_DIRS: &[&str] = &[
    "/etc/glvnd/egl_vendor.d",
    "/run/opengl-driver/share/glvnd/egl_vendor.d",
    "/usr/share/glvnd/egl_vendor.d",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Manifest {
    pub(crate) path: PathBuf,
    pub(crate) library: PathBuf,
}

impl Manifest {
    pub(crate) fn read(path: PathBuf) -> Option<Self> {
        let text = std::fs::read_to_string(&path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&text).ok()?;
        let named = Path::new(json.get("ICD")?.get("library_path")?.as_str()?);
        let library = match named.is_absolute() || named.parent() == Some(Path::new("")) {
            true => named.to_owned(),
            false => path.parent()?.join(named),
        };
        Some(Self { path, library })
    }

    pub(crate) fn named_like(&self, prefix: &str) -> bool {
        named_like(&self.path, prefix)
    }

    fn serves(&self, vendor: EglVendor) -> bool {
        self.library
            .file_name()
            .and_then(OsStr::to_str)
            .and_then(|name| name.strip_prefix(vendor.library()))
            .is_some_and(|rest| rest.starts_with(".so"))
    }
}

pub(crate) fn named_like(path: &Path, prefix: &str) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.starts_with(prefix))
}

pub(crate) fn serving(vendors: &[Manifest], vendor: EglVendor) -> Vec<PathBuf> {
    vendors
        .iter()
        .filter(|manifest| manifest.serves(vendor))
        .map(|manifest| manifest.path.clone())
        .collect()
}

pub(crate) fn vulkan_chosen_elsewhere() -> bool {
    VULKAN_CHOSEN_ELSEWHERE
        .iter()
        .any(|name| std::env::var_os(name).is_some())
}

pub(crate) fn vulkan_manifest_files() -> Vec<PathBuf> {
    let dirs = config_dirs()
        .into_iter()
        .chain([PathBuf::from(SYSTEM_CONFIG)])
        .chain(data_dirs())
        .chain([Path::new(NIXOS_DRIVERS).join("share")])
        .map(|root| root.join(VULKAN_MANIFESTS));
    json_files(dirs)
}

pub(crate) fn vulkan_manifests() -> Vec<Manifest> {
    vulkan_manifest_files()
        .into_iter()
        .filter_map(Manifest::read)
        .collect()
}

pub(crate) fn egl_vendors() -> Vec<Manifest> {
    let dirs: Vec<PathBuf> = match std::env::var_os(EGL_DIRS_ENV) {
        Some(dirs) => std::env::split_paths(&dirs).collect(),
        None => EGL_DEFAULT_DIRS.iter().map(PathBuf::from).collect(),
    };
    json_files(dirs)
        .into_iter()
        .filter_map(Manifest::read)
        .collect()
}

fn json_files(dirs: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut files = Vec::new();
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
        files.extend(paths);
    }
    files
}
