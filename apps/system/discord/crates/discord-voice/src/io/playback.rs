use std::sync::Arc;
use std::time::Duration;

use crate::io::attached::Attached;
use crate::io::{Devices, Speaker};

#[derive(Debug)]
pub(crate) struct Playback {
    devices: Arc<dyn Devices>,
    speaker: Attached<dyn Speaker>,
    failed: bool,
}

impl Playback {
    pub(crate) fn new(devices: Arc<dyn Devices>, device_name: Option<String>) -> Playback {
        Playback {
            devices,
            speaker: Attached::new(device_name),
            failed: false,
        }
    }

    pub(crate) fn choose(&mut self, device_name: Option<String>) {
        self.speaker.choose(device_name);
    }

    pub(crate) fn poll(&mut self) -> bool {
        let devices = &self.devices;
        let generation = devices.hotplug().generation();
        let lost = self.speaker.poll(generation, |name| devices.speaker(name));
        std::mem::take(&mut self.failed) || lost
    }

    pub(crate) fn queued(&mut self) -> Option<Duration> {
        self.speaker.device().map(|speaker| speaker.queued())
    }

    pub(crate) fn speaker(&mut self) -> Option<&dyn Speaker> {
        self.speaker.device().map(|speaker| &*speaker)
    }

    pub(crate) fn underruns(&mut self) -> u32 {
        self.speaker
            .device()
            .map_or(0, |speaker| speaker.underruns())
    }

    pub(crate) fn write(&mut self, stereo: &[f32]) {
        let Some(speaker) = self.speaker.device() else {
            return;
        };
        if let Err(error) = speaker.write(stereo) {
            tracing::warn!("audio output stopped: {error}");
            self.failed |= self.speaker.fail(self.devices.hotplug().generation());
        }
    }

    pub(crate) fn release(&mut self) {
        self.speaker.release();
    }
}
