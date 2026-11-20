use rikka_resample::ResampleQuality;

pub(crate) const RESAMPLE_QUALITY: ResampleQuality = ResampleQuality::Balanced;
pub(crate) const MAX_VOICES: usize = 64;
const BUFFER_FRAMES: u32 = 512;

#[derive(Debug, Clone, Default)]
#[must_use]
pub struct EngineConfig {
    pub(crate) device_name: Option<String>,
    pub(crate) sample_rate: Option<u32>,
    pub(crate) buffer_size: Option<u32>,
    pub(crate) channels: Option<u16>,
}

impl EngineConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn device_name(mut self, name: impl Into<String>) -> Self {
        self.device_name = Some(name.into());
        self
    }

    pub fn sample_rate(mut self, rate: u32) -> Self {
        self.sample_rate = Some(rate);
        self
    }

    pub fn buffer_size(mut self, frames: u32) -> Self {
        self.buffer_size = Some(frames);
        self
    }

    pub fn channels(mut self, channels: u16) -> Self {
        self.channels = Some(channels);
        self
    }

    pub(crate) fn effective_buffer_size(&self) -> u32 {
        self.buffer_size.unwrap_or(BUFFER_FRAMES)
    }
}
