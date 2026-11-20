use std::os::fd::AsFd;
use std::sync::Arc;

use rikka_worker::EventFd;

use crate::error::Result;
use crate::io::attached::Attached;
use crate::io::{Devices, Microphone};

const BACKLOG_FRAMES: usize = 12;

#[derive(Debug)]
pub(crate) struct Capture {
    devices: Arc<dyn Devices>,
    microphone: Attached<dyn Microphone>,
    pending: Vec<f32>,
    frame_len: usize,
}

impl Capture {
    pub(crate) fn new(
        devices: Arc<dyn Devices>,
        device_name: Option<String>,
        frame_len: usize,
    ) -> Capture {
        Capture {
            devices,
            microphone: Attached::new(device_name),
            pending: Vec::with_capacity(frame_len * (BACKLOG_FRAMES + 2)),
            frame_len,
        }
    }
    pub(crate) fn open(&mut self) -> Result<()> {
        let microphone = self.devices.microphone(self.microphone.device_name())?;
        self.microphone.attach(microphone);
        Ok(())
    }

    pub(crate) fn choose(&mut self, device_name: Option<String>) {
        self.microphone.choose(device_name);
    }

    pub(crate) fn poll(&mut self) -> bool {
        let devices = &self.devices;
        let generation = devices.hotplug().generation();
        let lost = self
            .microphone
            .poll(generation, |name| devices.microphone(name));
        if lost {
            self.pending.clear();
        }
        if let Some(microphone) = self.microphone.device() {
            microphone.read(&mut self.pending);
        }
        let limit = self.frame_len * BACKLOG_FRAMES;
        if self.pending.len() > limit {
            let excess = self.pending.len() - self.frame_len;
            let whole = excess - excess % self.frame_len;
            self.pending.drain(..whole);
        }
        lost
    }

    pub(crate) fn wait(&mut self, bell: &EventFd) -> std::io::Result<()> {
        match self.microphone.device() {
            Some(microphone) => bell.wait_readable(microphone.as_fd(), None).map(drop),
            None => bell.wait(),
        }
    }

    pub(crate) fn next_frame(&mut self, frame: &mut [f32]) -> bool {
        if self.pending.len() < self.frame_len || frame.len() != self.frame_len {
            return false;
        }
        frame.copy_from_slice(&self.pending[..self.frame_len]);
        self.pending.drain(..self.frame_len);
        true
    }
}
