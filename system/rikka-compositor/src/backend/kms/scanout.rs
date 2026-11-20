use std::collections::{HashSet, VecDeque};
use std::fmt;

use rikka_compositor_kms::{Device, Dmabuf, DmabufPlane, Framebuffer};
use rikka_compositor_planes::Layout;
use rikka_compositor_surface::{Dmabuf as ClientDmabuf, SurfaceId, Surfaces};
use rikka_compositor_types::Format;

use crate::render::{DmabufKey, Pin, Scenes, key_of};

const REMEMBERED: usize = 4;

#[derive(Default)]
pub(super) struct Scanout {
    framebuffers: VecDeque<(DmabufKey, Framebuffer)>,
    refused: HashSet<Format>,
    shown: Option<ScanoutState>,
    flights: Vec<Flight>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScanoutState {
    layout: Layout<SurfaceId>,
    generation: u64,
}

struct Flight {
    frame: u64,
    format: Format,
    pin: Pin,
}

impl Scanout {
    pub(super) fn refuses(&self, format: Format) -> bool {
        self.refused.contains(&format)
    }

    pub(super) fn shows(&self, layout: &Layout<SurfaceId>, generation: u64) -> bool {
        self.shown
            == Some(ScanoutState {
                layout: *layout,
                generation,
            })
    }

    pub(super) fn is_active(&self) -> bool {
        self.shown.is_some()
    }

    pub(super) fn framebuffer(
        &mut self,
        device: &Device,
        dmabuf: &ClientDmabuf,
    ) -> Option<Framebuffer> {
        let format = dmabuf.format();
        let key = key_of(dmabuf)?;
        if let Some(index) = self
            .framebuffers
            .iter()
            .position(|(framebuffer_key, _)| *framebuffer_key == key)
        {
            let entry = self.framebuffers.remove(index)?;
            let framebuffer = entry.1.clone();
            self.framebuffers.push_front(entry);
            return Some(framebuffer);
        }
        let imported = scanned(dmabuf)
            .and_then(|dmabuf| device.import(&dmabuf).map_err(|error| error.to_string()));
        match imported {
            Ok(framebuffer) => {
                self.framebuffers.push_front((key, framebuffer.clone()));
                self.framebuffers.truncate(REMEMBERED);
                Some(framebuffer)
            }
            Err(error) => {
                tracing::debug!(
                    ?format,
                    "cannot scan out client buffer; compositing: {error}"
                );
                self.refused.insert(format);
                None
            }
        }
    }

    pub(super) fn presented(
        &mut self,
        frame: u64,
        layout: Layout<SurfaceId>,
        generation: u64,
        pin: Pin,
    ) {
        self.shown = Some(ScanoutState { layout, generation });
        self.flights.push(Flight {
            frame,
            format: layout.format,
            pin,
        });
    }

    pub(super) fn composited(&mut self) {
        if self.is_active() {
            self.framebuffers.clear();
        }
        self.shown = None;
    }

    pub(super) fn rejected(&mut self, frame: u64) -> bool {
        let Some(flight) = self.flights.iter().find(|flight| flight.frame == frame) else {
            return false;
        };
        tracing::debug!(
            format = ?flight.format,
            "scanout rejected client buffer; compositing this format from now on"
        );
        self.refused.insert(flight.format);
        self.shown = None;
        self.framebuffers.clear();
        true
    }

    pub(super) fn retired(&mut self, frame: u64, scenes: &mut Scenes, surfaces: &mut Surfaces) {
        for flight in self.flights.extract_if(.., |flight| flight.frame == frame) {
            scenes.unpin(flight.pin, surfaces);
        }
    }

    pub(super) fn reset(&mut self) {
        self.refused.clear();
        self.framebuffers.clear();
        self.shown = None;
    }

    pub(super) fn release(&mut self, scenes: &mut Scenes, surfaces: &mut Surfaces) {
        self.reset();
        for flight in self.flights.drain(..) {
            scenes.unpin(flight.pin, surfaces);
        }
    }
}

fn scanned(dmabuf: &ClientDmabuf) -> Result<Dmabuf, String> {
    let planes = dmabuf
        .planes()
        .iter()
        .map(|plane| {
            Ok(DmabufPlane {
                fd: plane.fd().try_clone_to_owned()?,
                offset: plane.offset(),
                stride: plane.stride(),
            })
        })
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|error| error.to_string())?;
    let size = dmabuf.size();
    Ok(Dmabuf::new(
        size.w.max(0).unsigned_abs(),
        size.h.max(0).unsigned_abs(),
        dmabuf.format(),
        planes,
    ))
}

impl fmt::Debug for Scanout {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Scanout")
            .field("framebuffers", &self.framebuffers.len())
            .field("refused", &self.refused)
            .field("shown", &self.shown)
            .field("flights", &self.flights.len())
            .finish()
    }
}
