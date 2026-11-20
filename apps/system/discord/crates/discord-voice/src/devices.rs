use rikka_audio::{AudioCapture, AudioEngine, CaptureKind};

use crate::error::Result;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub name: String,
    pub is_default: bool,
}

pub fn input_devices() -> Result<Vec<Device>> {
    Ok(AudioCapture::devices()?
        .into_iter()
        .filter(|device| device.kind == CaptureKind::Microphone)
        .map(|device| Device {
            name: device.name,
            is_default: device.is_default,
        })
        .collect())
}

pub fn output_devices() -> Result<Vec<Device>> {
    Ok(AudioEngine::devices()?
        .into_iter()
        .map(|device| Device {
            name: device.name,
            is_default: device.is_default,
        })
        .collect())
}
