use std::path::Path;

use ash::vk;
use libloading::Library;
use rikka_gpu_drivers::Drivers;

use crate::error::{Error, Result};

const ENTRY: &std::ffi::CStr = c"vk_icdGetInstanceProcAddr";

pub(crate) struct DirectDrivers {
    infos: Vec<vk::DirectDriverLoadingInfoLUNARG<'static>>,
    _libraries: Vec<Library>,
}

impl DirectDrivers {
    pub(crate) fn load() -> Result<Option<Self>> {
        let Some(paths) = Drivers::shipped().and_then(|drivers| drivers.vulkan_libraries()) else {
            return Ok(None);
        };
        let mut libraries = Vec::new();
        let mut infos = Vec::new();
        let mut loaded = Vec::new();
        let mut failure = None;
        for path in paths {
            match unsafe { open(&path) } {
                Ok((library, entry)) => {
                    infos.push(
                        vk::DirectDriverLoadingInfoLUNARG::default()
                            .pfn_get_instance_proc_addr(Some(entry)),
                    );
                    libraries.push(library);
                    loaded.push(path.display().to_string());
                }
                Err(error) => {
                    tracing::warn!("failed to load Vulkan driver {}: {error}", path.display());
                    failure = Some(Error::DriverLoading {
                        path,
                        source: error.into(),
                    });
                }
            }
        }
        if let Some(failure) = failure.filter(|_| infos.is_empty()) {
            return Err(failure);
        }
        tracing::info!("Vulkan drivers loaded directly: {}", loaded.join(", "));
        Ok(Some(Self {
            infos,
            _libraries: libraries,
        }))
    }

    pub(crate) fn infos(&self) -> &[vk::DirectDriverLoadingInfoLUNARG<'static>] {
        &self.infos
    }
}

unsafe fn open(
    path: &Path,
) -> std::result::Result<(Library, vk::PFN_vkGetInstanceProcAddr), libloading::Error> {
    let library = unsafe { Library::new(path) }?;
    let entry =
        *unsafe { library.get::<vk::PFN_vkGetInstanceProcAddr>(ENTRY.to_bytes_with_nul()) }?;
    Ok((library, entry))
}
