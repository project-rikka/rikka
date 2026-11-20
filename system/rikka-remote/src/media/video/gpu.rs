use rikka_sync::{Mutex, MutexGuard};
use std::sync::OnceLock;

use rikka_vulkan_video::{Codec, Encoder};

use crate::media::MediaError;

const APP: &str = "rikka-remote";
const PREFERENCE: [Codec; 2] = [Codec::H265, Codec::H264];

static DEVICE: Mutex<Option<rikka_gpu::Gpu>> = Mutex::new(None);
static CODECS: OnceLock<Vec<Codec>> = OnceLock::new();

pub(crate) struct Gpu(MutexGuard<'static, Option<rikka_gpu::Gpu>>);

impl Gpu {
    pub(crate) fn lock() -> Result<Gpu, MediaError> {
        let mut held = DEVICE.lock();
        if held.is_none() {
            let built = rikka_gpu::Gpu::builder()
                .app_name(APP)
                .compute_support()
                .video_encode()
                .build()
                .map_err(MediaError::Gpu)?;
            *held = Some(built);
        }
        Ok(Gpu(held))
    }

    pub(crate) fn device(&self) -> &rikka_gpu::Gpu {
        self.0
            .as_ref()
            .expect("the device is built when the lock is taken")
    }
}

pub(crate) fn codecs() -> Result<Vec<Codec>, MediaError> {
    if let Some(found) = CODECS.get() {
        return Ok(found.clone());
    }
    let gpu = Gpu::lock()?;
    let found: Vec<Codec> = PREFERENCE
        .into_iter()
        .filter(|codec| Encoder::supports(gpu.device(), *codec))
        .collect();
    tracing::info!(?found, "codecs this desktop encodes with Vulkan Video");
    Ok(CODECS.get_or_init(|| found).clone())
}
