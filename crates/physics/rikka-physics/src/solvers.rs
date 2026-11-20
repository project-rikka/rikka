use rikka_gpu::Compute;
use rikka_gpu::{Batch, Ticket};
use rikka_model_core::Instance;
use rikka_physics_fluid::{BodyId, Fluid, Inputs};
use rikka_physics_tailor::{Grab, Wind};

use crate::cloth_scene::ClothScene;
use crate::error::Result;
use crate::fluid_bodies::{add_bodies, body_of};
use crate::layout::Layout;
use crate::scene_frame::SceneFrame;
use crate::snapshot::{Checkpoint, PendingCapture, PendingCheckpoint, Recorded};

pub(crate) struct StepInputs<'a> {
    pub(crate) time: f64,
    pub(crate) wind: Wind,
    pub(crate) figures: &'a [Instance],
    pub(crate) grabs: Vec<(usize, Grab)>,
}

pub(crate) struct Solvers {
    compute: Compute,
    pub(crate) fluid: Fluid,
    pub(crate) bodies: Vec<(usize, BodyId)>,
    cloth_scene: ClothScene,
}

impl Solvers {
    pub(crate) fn build(layout: &Layout, compute: &Compute) -> Result<Solvers> {
        let mut fluid = layout.fluid.clone().build(compute)?;
        let bodies = add_bodies(layout, |model| fluid.add_body(model))?;
        Ok(Solvers {
            compute: compute.clone(),
            fluid,
            bodies,
            cloth_scene: ClothScene::build(layout, compute)?,
        })
    }

    pub(crate) fn batch(&self) -> Batch {
        self.compute.batch()
    }

    pub(crate) fn reset(&mut self) -> Result<()> {
        self.fluid.reset()?;
        if let Some(cloth) = self.cloth_scene.cloth.as_mut() {
            cloth.reset()?;
        }
        Ok(())
    }

    pub(crate) fn record_step(
        &mut self,
        batch: &mut Batch,
        step_inputs: StepInputs<'_>,
        inputs: Inputs,
    ) -> Result<Recorded> {
        let cloth = self.drape(batch, &step_inputs)?;
        let inputs = self.draped(inputs)?;
        let fluid = self.fluid.advance_into(batch, step_inputs.time, &inputs)?;
        Ok(Recorded { fluid, cloth })
    }

    pub(crate) fn commit(&mut self, recorded: Recorded, ready: &Ticket) {
        self.fluid.commit(recorded.fluid, ready);
        if let (Some(cloth), Some(drapes)) = (self.cloth_scene.cloth.as_mut(), recorded.cloth) {
            cloth.commit(drapes, ready);
        }
    }

    fn drape(
        &mut self,
        batch: &mut Batch,
        step_inputs: &StepInputs<'_>,
    ) -> Result<Option<rikka_physics_cloth::Recorded>> {
        let Some(cloth) = self.cloth_scene.cloth.as_mut() else {
            return Ok(None);
        };
        let mut inputs = self.cloth_scene.figures.inputs(
            step_inputs.wind,
            |index| step_inputs.figures.get(index),
            &step_inputs.grabs,
        );
        for (index, figure) in self.cloth_scene.figures.iter() {
            let Some(body) = body_of(&self.bodies, index) else {
                continue;
            };
            for (node, _) in cloth.recorded_drapes(figure)? {
                if let Some(reaction) = self.fluid.reaction(body, node) {
                    inputs = inputs.reaction(figure, node, &reaction);
                }
            }
        }
        Ok(Some(cloth.advance_into(
            batch,
            step_inputs.time,
            &inputs,
        )?))
    }

    fn draped(&self, mut inputs: Inputs) -> Result<Inputs> {
        let Some(cloth) = self.cloth_scene.cloth.as_ref() else {
            return Ok(inputs);
        };
        for (index, figure) in self.cloth_scene.figures.iter() {
            let Some(body) = body_of(&self.bodies, index) else {
                continue;
            };
            for (node, buffer) in cloth.recorded_drapes(figure)? {
                inputs = inputs.drape(body, node, buffer);
            }
        }
        Ok(inputs)
    }

    pub(crate) fn capture_into(&self, batch: &mut Batch) -> Result<PendingCapture> {
        Ok(PendingCapture {
            fluid: self.fluid.capture_into(batch)?,
            cloth: self
                .cloth_scene
                .cloth
                .as_ref()
                .map(|cloth| cloth.capture_into(batch))
                .transpose()?,
        })
    }

    pub(crate) fn checkpoint_into(&mut self, batch: &mut Batch) -> PendingCheckpoint<'_> {
        PendingCheckpoint {
            fluid: self.fluid.checkpoint_into(batch),
            cloth: self
                .cloth_scene
                .cloth
                .as_mut()
                .map(|cloth| cloth.checkpoint_into(batch)),
        }
    }

    pub(crate) fn restore(&mut self, checkpoint: &Checkpoint) -> Result<()> {
        let mut batch = self.compute.batch();
        let fluid = self.fluid.restore_into(&mut batch, &checkpoint.fluid)?;
        let cloth = match (self.cloth_scene.cloth.as_mut(), &checkpoint.cloth) {
            (Some(cloth), Some(checkpoint)) => Some(cloth.restore_into(&mut batch, checkpoint)?),
            _ => None,
        };
        let ready = batch.submit()?;
        self.commit(Recorded { fluid, cloth }, &ready);
        Ok(())
    }

    pub(crate) fn frame(&self, layout: &Layout) -> SceneFrame {
        SceneFrame::new(
            self.fluid.surface(),
            layout,
            &self.bodies,
            |body| self.fluid.wetness(body),
            self.cloth_scene.drapes(layout),
        )
    }
}
