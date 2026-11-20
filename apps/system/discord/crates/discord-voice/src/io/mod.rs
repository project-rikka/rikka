mod attached;
mod capture;
mod hotplug;
mod playback;
mod system;

use std::os::fd::AsFd;
use std::time::Duration;

use crate::error::Result;

pub(crate) use capture::Capture;
pub(crate) use hotplug::Hotplug;
pub(crate) use playback::Playback;
pub(crate) use system::System;

pub(crate) trait Endpoint: Send + std::fmt::Debug {
    fn is_lost(&self) -> bool;
}

pub(crate) trait Microphone: Endpoint + AsFd {
    fn read(&mut self, into: &mut Vec<f32>);
}

pub(crate) trait Speaker: Endpoint + AsFd {
    fn queued(&self) -> Duration;

    fn watch_queued_below(&self, threshold: Duration) -> bool;

    fn underruns(&self) -> u32;

    fn write(&mut self, stereo: &[f32]) -> Result<()>;
}

pub(crate) trait Devices: Send + Sync + std::fmt::Debug {
    fn microphone(&self, name: Option<&str>) -> Result<Box<dyn Microphone>>;

    fn speaker(&self, name: Option<&str>) -> Result<Box<dyn Speaker>>;

    fn hotplug(&self) -> &Hotplug;
}
