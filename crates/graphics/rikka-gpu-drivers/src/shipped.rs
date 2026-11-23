use std::cell::OnceCell;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use rikka_shipped::Shipped;

use crate::extend_unique;
use crate::kernel::{EglVendor, Kernel, machine, mesa_manifest_prefix};
use crate::manifest::{
    EGL_FILENAMES_ENV, Manifest, NIXOS_DRIVERS, VULKAN_FILES_ENV, VULKAN_MANIFESTS, egl_vendors,
    serving, vulkan_chosen_elsewhere, vulkan_manifests,
};
use crate::system::VULKAN_SELECT_ENV;

const MESA: Shipped = Shipped {
    variable: "RIKKA_MESA_PATH",
    installed: "lib/rikka-mesa",
};

const GBM_BACKENDS_ENV: &str = "GBM_BACKENDS_PATH";

const VA_DRIVERS_ENV: &str = "LIBVA_DRIVERS_PATH";

const GBM_BACKENDS: &str = "lib/gbm";

const VA_DRIVERS: &str = "lib/dri";

const MESA_EGL_VENDOR: &str = "share/glvnd/egl_vendor.d/50_mesa.json";

#[derive(Debug)]
pub struct Drivers {
    kernels: Vec<&'static Kernel>,
    mesa: PathBuf,
}

impl Drivers {
    #[must_use]
    pub fn shipped() -> Option<Self> {
        let mesa = MESA.folder()?;
        let kernels = machine().kernels;
        (!kernels.is_empty()).then_some(Self { kernels, mesa })
    }

    #[must_use]
    pub fn vulkan_libraries(&self) -> Option<Vec<PathBuf>> {
        let manifests = self.vulkan()?;
        Some(
            manifests
                .into_iter()
                .map(|manifest| manifest.library)
                .collect(),
        )
    }

    #[must_use]
    pub fn environment(&self) -> Vec<(&'static str, Option<OsString>)> {
        let mut environment = Vec::new();
        for (name, shipped) in [
            (GBM_BACKENDS_ENV, GBM_BACKENDS),
            (VA_DRIVERS_ENV, VA_DRIVERS),
        ] {
            if let Ok(paths) = std::env::join_paths([
                self.mesa.join(shipped),
                Path::new(NIXOS_DRIVERS).join(shipped),
            ]) {
                environment.push((name, Some(paths)));
            }
        }
        if let Some(files) = self
            .vulkan()
            .and_then(|manifests| std::env::join_paths(manifests.iter().map(|m| &m.path)).ok())
        {
            environment.push((VULKAN_FILES_ENV, Some(files)));
            environment.push((VULKAN_SELECT_ENV, None));
        }
        if let Some(files) = self
            .egl_vendors()
            .and_then(|files| std::env::join_paths(files).ok())
        {
            environment.push((EGL_FILENAMES_ENV, Some(files)));
        }
        environment
    }

    fn vulkan(&self) -> Option<Vec<Manifest>> {
        if vulkan_chosen_elsewhere() {
            return None;
        }
        let arch = std::env::consts::ARCH;
        let system = OnceCell::new();
        let mut chosen: Vec<Manifest> = Vec::new();
        for kernel in &self.kernels {
            let mut found: Vec<Manifest> = kernel
                .mesa_vulkan
                .iter()
                .filter_map(|name| {
                    let file = format!("{}.{arch}.json", mesa_manifest_prefix(name));
                    Manifest::read(self.mesa.join("share").join(VULKAN_MANIFESTS).join(file))
                })
                .collect();
            if found.is_empty() {
                found = system
                    .get_or_init(vulkan_manifests)
                    .iter()
                    .filter(|manifest| {
                        kernel
                            .vendor_vulkan
                            .iter()
                            .any(|prefix| manifest.named_like(prefix))
                    })
                    .cloned()
                    .collect();
            }
            if found.is_empty() {
                tracing::debug!("no Vulkan driver for kernel driver {}", kernel.name);
                return None;
            }
            extend_unique(&mut chosen, found);
        }
        Some(chosen)
    }

    fn egl_vendors(&self) -> Option<Vec<PathBuf>> {
        let system = OnceCell::new();
        let mut chosen: Vec<PathBuf> = Vec::new();
        for kernel in &self.kernels {
            let found = kernel
                .egl
                .iter()
                .map(|&vendor| match vendor {
                    EglVendor::Mesa => Some(self.mesa.join(MESA_EGL_VENDOR))
                        .filter(|path| path.is_file())
                        .into_iter()
                        .collect(),
                    EglVendor::Nvidia => serving(system.get_or_init(egl_vendors), vendor),
                })
                .find(|paths: &Vec<PathBuf>| !paths.is_empty())?;
            extend_unique(&mut chosen, found);
        }
        Some(chosen)
    }
}
