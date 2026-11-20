#[cfg(unix)]
use std::os::fd::OwnedFd;
use std::sync::Arc;

use rikka_types::{Rect, Size};

use crate::consume::ImportedConsumer;
use crate::handoff::Handoff;
use crate::lease::Lease;
use crate::pulse::Pulse;
use crate::repaint::merged;

#[derive(Clone)]
pub struct ImportedBuffer(Arc<Handoff<ImportedFrame>>);

#[derive(Debug)]
pub struct ImportedFrame {
    pub buffer: u64,
    pub width: u32,
    pub height: u32,
    pub fourcc: u32,
    pub modifier: u64,
    pub planes: Vec<ImportedPlane>,
    pub lease: Option<Lease>,
    pub damage: Option<Vec<Rect>>,
    pub pulse: Option<Pulse>,
}

#[cfg(unix)]
#[derive(Debug)]
pub struct ImportedPlane {
    pub fd: OwnedFd,
    pub offset: u32,
    pub stride: u32,
}

#[cfg(windows)]
#[derive(Debug)]
pub enum ImportedPlane {}

impl Default for ImportedBuffer {
    fn default() -> ImportedBuffer {
        ImportedBuffer::new()
    }
}

impl ImportedBuffer {
    #[must_use]
    pub fn new() -> ImportedBuffer {
        ImportedBuffer(Handoff::new())
    }

    pub fn show(&self, arrival: ImportedFrame) {
        if arrival.width == 0 || arrival.height == 0 || arrival.planes.is_empty() {
            tracing::warn!(
                "invalid {}x{} frame with {} planes",
                arrival.width,
                arrival.height,
                arrival.planes.len()
            );
            return;
        }
        let (width, height) = (arrival.width, arrival.height);
        self.0
            .give_after(width, height, arrival, |arrival, untaken| {
                arrival.damage = merged(untaken.damage.take(), arrival.damage.take());
                arrival.pulse = arrival.pulse.take().or(untaken.pulse.take());
            });
    }

    #[must_use]
    pub fn size(&self) -> Option<Size> {
        self.0.size()
    }

    #[must_use]
    pub fn id(&self) -> u64 {
        self.0.id()
    }
}

impl ImportedConsumer for ImportedBuffer {
    fn take(&self) -> Option<ImportedFrame> {
        self.0.take()
    }
}

impl std::fmt::Debug for ImportedBuffer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImportedBuffer")
            .field("id", &self.0.id())
            .field("size", &self.size())
            .finish_non_exhaustive()
    }
}
