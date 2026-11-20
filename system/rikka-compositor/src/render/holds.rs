use std::collections::HashMap;

use rikka_compositor_surface::{BufferHold, SurfaceId, Surfaces};

#[derive(Debug)]
struct HoldEntry {
    id: u64,
    hold: BufferHold,
    read_by: u64,
    shown: usize,
}

#[derive(Debug, PartialEq, Eq)]
#[must_use = "pinned scanout buffers stay held until released"]
pub(crate) struct Pin(u64);

#[derive(Debug, Default)]
pub(crate) struct GpuHolds {
    current: HashMap<SurfaceId, HoldEntry>,
    retiring: Vec<HoldEntry>,
    finished: u64,
    next: u64,
}

impl GpuHolds {
    pub(crate) fn hold(&mut self, surface: SurfaceId, hold: Option<BufferHold>) {
        let previous = match hold {
            Some(hold) => {
                self.next += 1;
                self.current.insert(
                    surface,
                    HoldEntry {
                        id: self.next,
                        hold,
                        read_by: 0,
                        shown: 0,
                    },
                )
            }
            None => self.current.remove(&surface),
        };
        self.retiring.extend(previous);
    }

    pub(crate) fn read(&mut self, surface: SurfaceId, frame: u64) {
        if let Some(entry) = self.current.get_mut(&surface) {
            entry.read_by = entry.read_by.max(frame);
        }
    }

    pub(crate) fn pin(&mut self, surface: SurfaceId) -> Option<Pin> {
        let entry = self.current.get_mut(&surface)?;
        entry.shown += 1;
        Some(Pin(entry.id))
    }

    pub(crate) fn unpin(&mut self, pin: Pin, surfaces: &mut Surfaces) {
        if let Some(entry) = self
            .current
            .values_mut()
            .chain(self.retiring.iter_mut())
            .find(|entry| entry.id == pin.0)
        {
            entry.shown = entry.shown.saturating_sub(1);
        }
        self.release(self.finished, surfaces);
    }

    pub(crate) fn awaited(&self) -> Option<u64> {
        self.retiring
            .iter()
            .filter(|entry| entry.read_by > self.finished)
            .map(|entry| entry.read_by)
            .max()
    }

    pub(crate) fn release(&mut self, finished: u64, surfaces: &mut Surfaces) {
        self.finished = self.finished.max(finished);
        let finished = self.finished;
        let (done, waiting): (Vec<HoldEntry>, Vec<HoldEntry>) = std::mem::take(&mut self.retiring)
            .into_iter()
            .partition(|entry| entry.read_by <= finished && entry.shown == 0);
        self.retiring = waiting;
        for entry in done {
            surfaces.release(entry.hold);
        }
    }
}
