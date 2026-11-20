use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};

use ash::vk;
use rikka_sync::Mutex;

use crate::error::{Error, Result};

const HEADER_SIZE: usize = 32;

const LARGEST_READ: u64 = 512 * 1024 * 1024;
const CACHE_FOLDER: &str = "renderer";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Identity {
    vendor_id: u32,
    device_id: u32,
    uuid: [u8; vk::UUID_SIZE],
}

impl Identity {
    fn from_properties(properties: &vk::PhysicalDeviceProperties) -> Self {
        Self {
            vendor_id: properties.vendor_id,
            device_id: properties.device_id,
            uuid: properties.pipeline_cache_uuid,
        }
    }

    fn accepts(&self, data: &[u8]) -> bool {
        if data.len() < HEADER_SIZE {
            return false;
        }
        let word = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().expect("4 bytes"));
        let header_size = word(0) as usize;
        header_size >= HEADER_SIZE
            && header_size <= data.len()
            && word(4) == vk::PipelineCacheHeaderVersion::ONE.as_raw() as u32
            && word(8) == self.vendor_id
            && word(12) == self.device_id
            && data[16..32] == self.uuid
    }

    fn file_name(&self) -> String {
        format!(
            "pipelines-{:04x}-{:04x}.bin",
            self.vendor_id, self.device_id
        )
    }
}

pub(crate) struct PipelineCache {
    handle: vk::PipelineCache,
    persisted: Arc<PersistentCache>,
}

pub(crate) struct PersistentCache {
    device: ash::Device,
    handle: vk::PipelineCache,
    path: Option<PathBuf>,
    saved: Mutex<Option<usize>>,
}

impl PipelineCache {
    pub(crate) fn open(
        device: &ash::Device,
        properties: &vk::PhysicalDeviceProperties,
    ) -> Result<Self> {
        let identity = Identity::from_properties(properties);
        let path = cache_directory().map(|directory| directory.join(identity.file_name()));
        let initial = path
            .as_deref()
            .and_then(read_bounded)
            .filter(|data| {
                let accepted = identity.accepts(data);
                if !accepted {
                    tracing::debug!("ignoring pipeline cache from another driver");
                }
                accepted
            })
            .unwrap_or_default();

        let info = vk::PipelineCacheCreateInfo::default().initial_data(&initial);
        let handle = match unsafe { device.create_pipeline_cache(&info, None) } {
            Ok(handle) => handle,
            Err(err) if !initial.is_empty() => {
                tracing::warn!("driver rejected saved pipeline cache: {err}");
                unsafe {
                    device.create_pipeline_cache(&vk::PipelineCacheCreateInfo::default(), None)?
                }
            }
            Err(err) => return Err(err.into()),
        };
        if !initial.is_empty() {
            tracing::debug!(bytes = initial.len(), "pipeline cache restored");
        }

        Ok(Self {
            handle,
            persisted: Arc::new(PersistentCache {
                device: device.clone(),
                handle,
                path,
                saved: Mutex::new(Some(initial.len())),
            }),
        })
    }

    pub(crate) fn handle(&self) -> vk::PipelineCache {
        self.handle
    }

    pub(crate) fn persisted(&self) -> Weak<PersistentCache> {
        Arc::downgrade(&self.persisted)
    }

    pub(crate) fn save(&self) -> Result<()> {
        self.persisted.save()
    }

    pub(crate) unsafe fn close(&self) {
        let mut saved = self.persisted.saved.lock();
        if let Err(error) = self.persisted.write(&mut saved) {
            tracing::warn!(%error, "failed to save pipeline cache");
        }
        *saved = None;
        unsafe {
            self.persisted
                .device
                .destroy_pipeline_cache(self.handle, None)
        };
    }
}

impl PersistentCache {
    pub(crate) fn save(&self) -> Result<()> {
        self.write(&mut self.saved.lock())
    }

    fn write(&self, saved: &mut Option<usize>) -> Result<()> {
        let (Some(path), Some(written)) = (&self.path, *saved) else {
            return Ok(());
        };
        if self.size()? <= written {
            return Ok(());
        }
        let data = unsafe { self.device.get_pipeline_cache_data(self.handle) }?;
        rikka_fs::replace(path, &data).map_err(|source| Error::PipelineCache {
            path: path.clone(),
            source,
        })?;
        *saved = Some(data.len());
        tracing::debug!(bytes = data.len(), path = %path.display(), "pipeline cache saved");
        Ok(())
    }

    fn size(&self) -> Result<usize> {
        let mut size = 0usize;
        unsafe {
            (self.device.fp_v1_0().get_pipeline_cache_data)(
                self.device.handle(),
                self.handle,
                &mut size,
                std::ptr::null_mut::<c_void>(),
            )
        }
        .result()?;
        Ok(size)
    }
}

fn cache_directory() -> Option<PathBuf> {
    let paths = rikka_dirs::Paths::resolve().ok()?;
    Some(paths.cache_dir().join(CACHE_FOLDER))
}

fn read_bounded(path: &Path) -> Option<Vec<u8>> {
    let length = std::fs::metadata(path).ok()?.len();
    if length > LARGEST_READ {
        return None;
    }
    std::fs::read(path).ok()
}
