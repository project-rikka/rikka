pub(crate) mod cards;
mod descriptors;
mod formats;
mod mixer;
mod pcm;
pub(crate) mod probe;
mod realtime;
mod refine;
pub(crate) mod reserve;
mod writer;

use std::sync::Arc;

use rikka_hifi_core::{Carriage, ExclusivePath, Plan};
use rikka_worker::Worker;

use crate::device::Device;
use crate::event::Refusal;
use crate::notice::Notifier;
use crate::ring::{self, RingWriter};
use reserve::Reservation;
use writer::Control;

pub(crate) use mixer::HardwareVolume;

const BUFFERED_SECONDS_DIVISOR: usize = 5;

pub(crate) struct Claim {
    pub(crate) card: u32,
    pub(crate) volume: Option<HardwareVolume>,
    pub(crate) reservation: Option<Reservation>,
}

impl Claim {
    pub(crate) fn has_hardware_volume(&self) -> bool {
        self.volume.is_some()
    }

    pub(crate) fn set_volume(&self, level: f32) {
        if let Some(volume) = &self.volume {
            volume.set(level);
        }
    }
}

impl std::fmt::Debug for Claim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Claim")
            .field("card", &self.card)
            .field("volume", &self.volume)
            .field("reservation", &self.reservation)
            .finish()
    }
}

pub(crate) struct ExclusiveOutput {
    device: Device,
    plan: Plan,
    ring: RingWriter,
    writer: Worker<Arc<Control>>,
}

impl ExclusiveOutput {
    pub(crate) fn open(
        device: Device,
        plan: Plan,
        notifier: &Notifier,
    ) -> Result<ExclusiveOutput, Refusal> {
        let pcm_device = pcm::open(&device.id, &plan).map_err(|error| match error.errno() {
            libc::EBUSY => Refusal::Busy { holder: None },
            libc::ENOENT | libc::ENODEV => Refusal::Missing,
            libc::EINVAL => Refusal::Unsupported,
            _ => Refusal::Open(error.to_string()),
        })?;
        let frames = (plan.rate as usize / BUFFERED_SECONDS_DIVISOR).max(pcm_device.period * 2);
        let opened = ring::open(frames, plan.frame_bytes(), notifier.wake().clone())
            .map_err(|error| Refusal::Open(error.to_string()))?;
        let writer = writer::spawn(pcm_device, opened.reader, opened.filled, notifier.clone())
            .map_err(Refusal::Open)?;
        Ok(ExclusiveOutput {
            device,
            plan,
            ring: opened.writer,
            writer,
        })
    }

    pub(crate) fn device(&self) -> &Device {
        &self.device
    }

    pub(crate) fn plan(&self) -> Plan {
        self.plan
    }

    pub(crate) fn ring(&self) -> &RingWriter {
        &self.ring
    }

    pub(crate) fn ring_mut(&mut self) -> &mut RingWriter {
        &mut self.ring
    }

    pub(crate) fn set_paused(&self, paused: bool) {
        self.writer.handle().set_paused(paused);
    }

    pub(crate) fn path(&self) -> ExclusivePath {
        ExclusivePath {
            device: self.device.id.clone(),
            name: self.device.name.clone(),
            rate: self.plan.rate,
            channels: self.plan.channels,
            format: self.plan.format,
            dop: self.plan.carriage == Carriage::Dop,
        }
    }
}

impl std::fmt::Debug for ExclusiveOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExclusiveOutput")
            .field("device", &self.device.id)
            .field("plan", &self.plan)
            .finish_non_exhaustive()
    }
}
