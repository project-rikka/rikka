use glam::Vec3;
use rikka_gpu::Compute;
use rikka_gpu::{Batch, Ticket};
use rikka_model_core::Model;

use crate::body::{BodyId, BodyMesh};
use crate::capture::Capture;
use crate::config::Config;
use crate::error::Result;
use crate::recorded::Recorded;
use crate::stage::Stage;
use crate::surface::{Surface, Wetting};

pub struct Replay {
    stage: Stage,
}

impl Replay {
    pub(crate) fn new(compute: &Compute, config: Config, gravity: Vec3) -> Result<Replay> {
        Ok(Replay {
            stage: Stage::new(compute, config, gravity, |_| {})?,
        })
    }

    pub fn add_body(&mut self, model: &Model) -> Result<BodyId> {
        let mesh = BodyMesh::flatten(model)?;
        self.stage.bodies.add(&self.stage.compute, mesh)
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

    pub fn commit(&mut self, recorded: Recorded, ready: &Ticket) {
        self.stage.present(recorded.frame, ready);
    }

    #[must_use]
    pub fn surface(&self) -> Surface {
        self.stage.surface()
    }

    #[must_use]
    pub fn wetness(&self, body: BodyId) -> Option<Wetting> {
        self.stage.wetness(body)
    }

    #[must_use]
    pub fn radius(&self) -> f32 {
        self.stage.config.radius
    }
}

impl std::fmt::Debug for Replay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Replay")
            .field("radius", &self.stage.config.radius)
            .field("capacity", &self.stage.config.capacity)
            .finish_non_exhaustive()
    }
}
