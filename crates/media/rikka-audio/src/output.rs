use crate::config::EngineConfig;
use crate::error::{Result, Role};
use crate::frame::check_channels;

pub(crate) const DEFAULT_RATE: u32 = 48_000;
pub(crate) const DEFAULT_CHANNELS: u16 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputDevice {
    pub name: String,
    pub is_default: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct OutputPlan {
    pub(crate) rate: u32,
    pub(crate) channels: u16,
    pub(crate) quantum: u32,
}

impl OutputPlan {
    pub(crate) fn new(config: &EngineConfig, native_rate: Option<u32>) -> Result<OutputPlan> {
        let channels = config.channels.unwrap_or(DEFAULT_CHANNELS);
        check_channels(channels, Role::Output)?;
        let rate = config
            .sample_rate
            .or(native_rate)
            .filter(|rate| *rate > 0)
            .unwrap_or(DEFAULT_RATE);
        Ok(OutputPlan {
            rate,
            channels,
            quantum: config.effective_buffer_size(),
        })
    }
}
