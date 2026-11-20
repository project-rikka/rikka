use rikka_gpu::{Batch, ComputeBuffer, Ticket};
use rikka_renderer_3d::FLUID_SPRAY_BYTES as SPRAY_BYTES;

use crate::error::Result;
use crate::fluid::Fluid;
use crate::gpu::{RECORD_POSITION, RECORD_VELOCITY};
use crate::recorded::Recorded;
use crate::stage::Stage;

const WORD: u64 = 4;

#[must_use = "capture is complete only after it is finished"]
pub struct PendingCapture {
    capture: Capture,
    ready: Ticket,
}

impl PendingCapture {
    pub fn finish(self) -> Result<Capture> {
        self.ready.wait()?;
        Ok(self.capture)
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready.is_ready()
    }
}

impl std::fmt::Debug for PendingCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingCapture")
            .field("capture", &self.capture)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct Capture {
    particles: u32,
    spray: u32,
    wet: u64,
    buffer: Option<ComputeBuffer>,
}

struct Parts {
    positions: u64,
    velocities: u64,
    spray: u64,
    wet: u64,
}

impl Parts {
    fn new(particles: u32, spray: u32, wet: u64) -> Parts {
        Parts {
            positions: u64::from(particles) * RECORD_POSITION,
            velocities: u64::from(particles) * RECORD_VELOCITY,
            spray: u64::from(spray) * SPRAY_BYTES,
            wet,
        }
    }

    fn velocities_at(&self) -> u64 {
        self.positions
    }

    fn spray_at(&self) -> u64 {
        self.velocities_at() + self.velocities
    }

    fn wet_at(&self) -> u64 {
        self.spray_at() + self.spray
    }

    fn total(&self) -> u64 {
        self.wet_at() + self.wet
    }
}

impl Capture {
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.buffer
            .as_ref()
            .map_or(0, |buffer| buffer.size() as usize)
    }

    fn parts(&self) -> Parts {
        Parts::new(self.particles, self.spray, self.wet)
    }
}

impl std::fmt::Debug for Capture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Capture")
            .field("particles", &self.particles)
            .field("spray", &self.spray)
            .finish_non_exhaustive()
    }
}

fn copy(batch: &mut Batch, from: (&ComputeBuffer, u64), to: (&ComputeBuffer, u64), size: u64) {
    if size > 0 {
        batch.copy_buffer(from.0, from.1, to.0, to.1, size);
    }
}

impl Fluid {
    pub fn capture(&mut self) -> Result<PendingCapture> {
        let mut batch = self.stage.compute.batch();
        let capture = self.capture_into(&mut batch)?;
        let ready = batch.submit()?;
        Ok(PendingCapture { capture, ready })
    }

    pub fn capture_into(&self, batch: &mut Batch) -> Result<Capture> {
        let particles = self.clock.slots;
        let (spray_buffer, spray) = &self.stage.last_spray;
        let wet = self
            .stage
            .bodies
            .buffers
            .as_ref()
            .map_or(0, |buffers| u64::from(buffers.vertices) * WORD);
        let parts = Parts::new(particles, *spray, wet);
        let total = parts.total();
        let buffer = match total {
            0 => None,
            _ => Some(self.stage.compute.buffer(total)?),
        };
        if let Some(into) = &buffer {
            copy(
                batch,
                (&self.stage.look.positions, 0),
                (into, 0),
                parts.positions,
            );
            copy(
                batch,
                (&self.stage.look.velocities, 0),
                (into, parts.velocities_at()),
                parts.velocities,
            );
            copy(
                batch,
                (spray_buffer, 0),
                (into, parts.spray_at()),
                parts.spray,
            );
            if let Some(buffers) = &self.stage.bodies.buffers {
                copy(
                    batch,
                    (&buffers.wet_record, 0),
                    (into, parts.wet_at()),
                    parts.wet,
                );
            }
        }
        Ok(Capture {
            particles,
            spray: *spray,
            wet,
            buffer,
        })
    }

    pub fn show(&mut self, capture: &Capture) -> Result<()> {
        let mut batch = self.stage.compute.batch();
        let recorded = self.show_into(&mut batch, capture)?;
        let ready = batch.submit()?;
        self.commit(recorded, &ready);
        Ok(())
    }

    pub fn show_into(&mut self, batch: &mut Batch, capture: &Capture) -> Result<Recorded> {
        self.stage.show_into(batch, capture)
    }
}

impl Stage {
    pub(crate) fn show_into(&mut self, batch: &mut Batch, capture: &Capture) -> Result<Recorded> {
        let parts = capture.parts();
        let spray = self.spray_ring.acquire(&self.compute)?;
        if let Some(from) = &capture.buffer {
            copy(batch, (from, 0), (&self.look.positions, 0), parts.positions);
            copy(
                batch,
                (from, parts.velocities_at()),
                (&self.look.velocities, 0),
                parts.velocities,
            );
            copy(batch, (from, parts.spray_at()), (&spray, 0), parts.spray);
            if let Some(buffers) = &self.bodies.buffers
                && capture.wet == u64::from(buffers.vertices) * WORD
            {
                copy(
                    batch,
                    (from, parts.wet_at()),
                    (&buffers.wet_record, 0),
                    parts.wet,
                );
            }
        }
        let frame = self.render_record(batch, capture.particles, spray, capture.spray)?;
        Ok(Recorded { frame })
    }
}
