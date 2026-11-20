use glam::Mat4;
use rikka_gpu::{Batch, ComputeBuffer, Download};
use rikka_model_core::NodeId;
use rikka_physics_gpu::REACTION_BYTES;

use crate::body::BodyId;
use crate::error::{Error, Result};
use crate::fluid::{Clock, Fluid};
use crate::gpu::{BodyDrape, MAX_BONDS};
use crate::recorded::Recorded;

const VEC4: u64 = 16;
const WORD: u64 = 4;

pub struct Checkpoint {
    clock: Clock,
    particles: Vec<Vec<u8>>,
    bonds: Vec<Vec<Vec<u8>>>,
    motion: Vec<u8>,
    foam: Vec<Vec<u8>>,
    wet: Vec<u8>,
    bodies: Vec<Posed>,
    entries: (u32, u32),
}

struct Posed {
    body: BodyId,
    palette: Vec<Mat4>,
    drapes: Vec<BodyDrape>,
    morph_weights: Vec<f32>,
    reactions: Vec<(NodeId, Vec<u8>)>,
}

impl Checkpoint {
    #[must_use]
    pub fn bytes(&self) -> usize {
        let drape_bytes: usize = self
            .bodies
            .iter()
            .map(|posed| {
                posed
                    .drapes
                    .iter()
                    .map(|drape| drape.buffer.size() as usize)
                    .sum::<usize>()
                    + posed
                        .reactions
                        .iter()
                        .map(|(_, bytes)| bytes.len())
                        .sum::<usize>()
            })
            .sum();
        let bond_bytes: usize = self
            .bonds
            .iter()
            .flat_map(|planes| planes.iter())
            .map(Vec::len)
            .sum();
        self.particles
            .iter()
            .chain(&self.foam)
            .map(Vec::len)
            .sum::<usize>()
            + bond_bytes
            + self.motion.len()
            + self.wet.len()
            + drape_bytes
    }
}

impl std::fmt::Debug for Checkpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Checkpoint")
            .field("time", &self.clock.time)
            .field("bytes", &self.bytes())
            .finish_non_exhaustive()
    }
}

#[must_use = "checkpoint is complete only after its batch is submitted and it is finished"]
pub struct PendingCheckpoint<'a> {
    fluid: &'a mut Fluid,
    particles: Vec<Option<Download>>,
    bonds: Vec<Vec<Option<Download>>>,
    motion: Option<Download>,
    foam: Vec<(Option<Download>, u64)>,
    wet: Option<Download>,
    reactions: Vec<Vec<(NodeId, Download)>>,
}

impl PendingCheckpoint<'_> {
    pub fn finish(self) -> Result<Checkpoint> {
        let PendingCheckpoint {
            fluid,
            particles,
            bonds,
            motion,
            foam,
            wet,
            reactions,
        } = self;
        fluid.settle()?;
        let resident = u64::from(fluid.clock.measured.latest.foam.min(fluid.foam.capacity));
        let mut bodies = Vec::with_capacity(fluid.stage.bodies.list.len());
        for (body, reactions) in fluid.stage.bodies.list.iter().zip(reactions) {
            bodies.push(Posed {
                body: body.id,
                palette: body.palette.clone(),
                drapes: body.drapes.clone(),
                morph_weights: body.morph_weights.clone(),
                reactions: reactions
                    .into_iter()
                    .map(|(node, download)| Ok((node, download.wait()?)))
                    .collect::<Result<_>>()?,
            });
        }
        Ok(Checkpoint {
            clock: fluid.clock.clone(),
            particles: particles.into_iter().map(wait).collect::<Result<_>>()?,
            bonds: bonds
                .into_iter()
                .map(|planes| planes.into_iter().map(wait).collect::<Result<_>>())
                .collect::<Result<_>>()?,
            motion: wait(motion)?,
            foam: foam
                .into_iter()
                .map(|(download, stride)| resident_part(download, resident * stride))
                .collect::<Result<_>>()?,
            wet: wait(wet)?,
            bodies,
            entries: fluid.stage.bodies.entries(),
        })
    }
}

impl std::fmt::Debug for PendingCheckpoint<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingCheckpoint")
            .field("time", &self.fluid.clock.time)
            .finish_non_exhaustive()
    }
}

fn wait(download: Option<Download>) -> Result<Vec<u8>> {
    download.map_or(Ok(Vec::new()), |download| Ok(download.wait()?))
}

fn resident_part(download: Option<Download>, bytes: u64) -> Result<Vec<u8>> {
    let mut bytes_read = wait(download)?;
    let Ok(bytes) = usize::try_from(bytes) else {
        return Err(Error::ShortReadback(bytes_read.len()));
    };
    if bytes_read.len() < bytes {
        return Err(Error::ShortReadback(bytes_read.len()));
    }
    bytes_read.truncate(bytes);
    Ok(bytes_read)
}

fn fetch(batch: &mut Batch, buffer: &ComputeBuffer, bytes: u64) -> Option<Download> {
    fetch_at(batch, buffer, 0, bytes)
}

fn fetch_at(
    batch: &mut Batch,
    buffer: &ComputeBuffer,
    offset: u64,
    bytes: u64,
) -> Option<Download> {
    (bytes > 0).then(|| batch.download_range(buffer, offset, bytes))
}

impl Fluid {
    pub fn checkpoint(&mut self) -> Result<Checkpoint> {
        let mut batch = self.stage.compute.batch();
        let pending_checkpoint = self.checkpoint_into(&mut batch);
        batch.submit()?;
        pending_checkpoint.finish()
    }

    pub fn checkpoint_into(&mut self, batch: &mut Batch) -> PendingCheckpoint<'_> {
        let slots = u64::from(self.clock.slots);
        let stride = u64::from(self.particles.stride);
        let resident = u64::from(self.resident_spray.min(self.foam.capacity));
        let particles = self
            .particles
            .state()
            .persistent()
            .iter()
            .map(|(buffer, elem)| fetch(batch, buffer, slots * elem))
            .collect();
        let bonds = if self.clock.bonds_active {
            self.particles
                .state()
                .bond_planes()
                .iter()
                .map(|buffer| {
                    (0..MAX_BONDS)
                        .map(|plane| fetch_at(batch, buffer, plane * stride * WORD, slots * WORD))
                        .collect()
                })
                .collect()
        } else {
            Vec::new()
        };
        let motion = if self.clock.moved {
            fetch(batch, &self.particles.motion, slots * VEC4)
        } else {
            None
        };
        let foam = self
            .foam
            .persistent()
            .iter()
            .map(|(buffer, stride)| (fetch(batch, buffer, resident * stride), *stride))
            .collect();
        let wet = self
            .stage
            .bodies
            .buffers
            .as_ref()
            .and_then(|buffers| fetch(batch, &buffers.wet, u64::from(buffers.vertices) * VEC4));
        let reactions = self
            .stage
            .bodies
            .list
            .iter()
            .map(|body| {
                body.reactions
                    .iter()
                    .map(|(node, buffer)| (*node, batch.download_buffer(buffer)))
                    .collect()
            })
            .collect();
        PendingCheckpoint {
            fluid: self,
            particles,
            bonds,
            motion,
            foam,
            wet,
            reactions,
        }
    }

    pub fn restore(&mut self, checkpoint: &Checkpoint) -> Result<()> {
        let mut batch = self.stage.compute.batch();
        let recorded = self.restore_into(&mut batch, checkpoint)?;
        let ready = batch.submit()?;
        self.commit(recorded, &ready);
        Ok(())
    }

    pub fn restore_into(&mut self, batch: &mut Batch, checkpoint: &Checkpoint) -> Result<Recorded> {
        let matches = checkpoint.bodies.len() == self.stage.bodies.list.len()
            && checkpoint
                .bodies
                .iter()
                .zip(&self.stage.bodies.list)
                .all(|(posed, body)| {
                    posed.body == body.id && posed.palette.len() == body.palette.len()
                });
        if !matches {
            return Err(Error::CheckpointMismatch);
        }
        self.settle()?;
        self.particles.current = 0;
        self.foam.current = 0;
        self.clock = checkpoint.clock.clone();
        for (posed, body) in checkpoint.bodies.iter().zip(&mut self.stage.bodies.list) {
            body.palette.clone_from(&posed.palette);
            body.previous.clone_from(&posed.palette);
            body.drapes.clone_from(&posed.drapes);
            body.morph_weights.clone_from(&posed.morph_weights);
            body.previous_morph_weights.clone_from(&posed.morph_weights);
        }
        for (posed, body) in checkpoint.bodies.iter().zip(&mut self.stage.bodies.list) {
            body.reactions.clear();
            for (node, bytes) in &posed.reactions {
                let vertices = (bytes.len() as u64 / REACTION_BYTES) as u32;
                let buffer = body
                    .reaction_ring(*node, vertices)
                    .acquire(&self.stage.compute)?;
                batch.write(&buffer, 0, bytes);
                body.reactions.push((*node, buffer));
            }
        }
        let (requested, capacity) = checkpoint.entries;
        self.stage
            .bodies
            .set_entries(&self.stage.compute, requested, capacity)?;
        self.write_settings(batch);
        let stride = u64::from(self.particles.stride);
        for ((buffer, _), bytes) in self
            .particles
            .state()
            .persistent()
            .iter()
            .zip(&checkpoint.particles)
        {
            if !bytes.is_empty() {
                batch.write(buffer, 0, bytes);
            }
        }
        for (buffer, planes) in self
            .particles
            .state()
            .bond_planes()
            .iter()
            .zip(&checkpoint.bonds)
        {
            for (plane, bytes) in planes.iter().enumerate() {
                if !bytes.is_empty() {
                    batch.write(buffer, plane as u64 * stride * WORD, bytes);
                }
            }
        }
        for ((buffer, _), bytes) in self.foam.persistent().iter().zip(&checkpoint.foam) {
            if !bytes.is_empty() {
                batch.write(buffer, 0, bytes);
            }
        }
        if !checkpoint.motion.is_empty() {
            batch.write(&self.particles.motion, 0, &checkpoint.motion);
        }
        let foam = self.clock.measured.latest.foam.min(self.foam.capacity);
        let counters = [foam, 0, 0, 0];
        batch.write(&self.foam.counters, 0, bytemuck::cast_slice(&counters));
        if let Some(buffers) = &self.stage.bodies.buffers {
            batch.fill(&buffers.deposits, 0);
            if !checkpoint.wet.is_empty() {
                batch.write(&buffers.wet, 0, &checkpoint.wet);
            }
        }
        self.stage.bodies.skin(batch, &self.stage.kernels, false);
        self.stage.bodies.dry(batch, &self.stage.kernels, 1.0);
        let frame = self.render_state(batch, foam)?;
        self.resident_spray = foam;
        Ok(Recorded { frame })
    }
}
