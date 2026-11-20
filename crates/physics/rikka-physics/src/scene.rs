use std::collections::BTreeMap;
use std::sync::Arc;

use rikka_gpu::Compute;
use rikka_gpu::{Batch, Ticket};
use rikka_physics_fluid::{Inputs, RegionId};

use crate::emitter::{EmitterContext, EmitterState};
use crate::error::Result;
use crate::figures::Figures;
use crate::layout::Layout;
use crate::mix::Mix;
use crate::region;
use crate::scene_frame::SceneFrame;
use crate::scene_source::SceneSource;
use crate::snapshot::{Capture, Checkpoint, PendingCheckpoint, Recorded};
use crate::solvers::{Solvers, StepInputs};

pub(crate) struct BakeOptions<'a> {
    pub(crate) capture: bool,
    pub(crate) checkpoint: bool,
    pub(crate) wait_for: Option<&'a Ticket>,
}

pub(crate) struct BakedStep {
    pub(crate) step: u32,
    pub(crate) capture: Option<Capture>,
    pub(crate) checkpoint: Option<Checkpoint>,
    pub(crate) ready: Ticket,
}

pub struct Scene {
    layout: Arc<Layout>,
    solvers: Solvers,
    source: Arc<dyn SceneSource>,
    figures: Figures,
    mix: Mix,
    regions: Vec<(u32, RegionId)>,
    emitter_states: BTreeMap<u32, EmitterState>,
    step: Option<u32>,
}

impl Scene {
    pub fn build(compute: &Compute, source: Arc<dyn SceneSource>) -> Result<Scene> {
        let layout = Arc::new(source.layout().clone());
        let solvers = Solvers::build(&layout, compute)?;
        let figures = Figures::new(source.as_ref());
        let mut scene = Scene {
            layout,
            solvers,
            figures,
            source,
            mix: Mix::default(),
            regions: Vec::new(),
            emitter_states: BTreeMap::new(),
            step: None,
        };
        scene.prepare()?;
        Ok(scene)
    }

    #[must_use]
    pub fn step(&self) -> Option<u32> {
        self.step
    }

    pub fn advance(&mut self) -> Result<u32> {
        let mut batch = self.solvers.batch();
        let (step, recorded) = self.record(&mut batch)?;
        let ready = batch.submit()?;
        self.solvers.commit(recorded, &ready);
        Ok(step)
    }

    pub(crate) fn bake(&mut self, options: BakeOptions<'_>) -> Result<BakedStep> {
        let mut batch = self.solvers.batch();
        let (step, recorded) = self.record(&mut batch)?;
        let pending_capture = if options.capture {
            Some(self.solvers.capture_into(&mut batch)?)
        } else {
            None
        };
        let pending_checkpoint = options
            .checkpoint
            .then(|| self.solvers.checkpoint_into(&mut batch));
        if let Some(previous) = options.wait_for {
            previous.wait()?;
        }
        let ready = batch.submit()?;
        let checkpoint = pending_checkpoint
            .map(PendingCheckpoint::finish)
            .transpose()?;
        self.solvers.commit(recorded, &ready);
        Ok(BakedStep {
            step,
            capture: pending_capture.map(|pending_capture| pending_capture.submitted(&ready)),
            checkpoint,
            ready,
        })
    }

    pub(crate) fn capture(&self, wait_for: Option<&Ticket>) -> Result<(Capture, Ticket)> {
        let mut batch = self.solvers.batch();
        let pending_capture = self.solvers.capture_into(&mut batch)?;
        if let Some(previous) = wait_for {
            previous.wait()?;
        }
        let ready = batch.submit()?;
        Ok((pending_capture.submitted(&ready), ready))
    }

    fn record(&mut self, batch: &mut Batch) -> Result<(u32, Recorded)> {
        let step = self.next_step();
        let time = self.layout.step_rate.time_of(step);
        self.figures.pose(self.source.as_ref(), time as f32);
        let figures = self.figures.instances();
        let step_inputs = StepInputs {
            time,
            wind: self.source.wind(time as f32),
            figures,
            grabs: self.source.grabs(time as f32, figures),
        };
        let emitter_states = self.emitter_states_at(step);
        let inputs = self.inputs(&emitter_states);
        let recorded = self.solvers.record_step(batch, step_inputs, inputs)?;
        self.emitter_states = emitter_states;
        self.step = Some(step);
        Ok((step, recorded))
    }

    #[must_use]
    pub fn frame(&self) -> SceneFrame {
        self.solvers.frame(&self.layout)
    }

    pub(crate) fn next_step(&self) -> u32 {
        self.step.map_or(0, |step| step + 1)
    }

    pub(crate) fn layout(&self) -> &Arc<Layout> {
        &self.layout
    }

    pub(crate) fn source(&self) -> Arc<dyn SceneSource> {
        Arc::clone(&self.source)
    }

    pub(crate) fn matches_layout(&self, source: &dyn SceneSource) -> bool {
        *self.layout == *source.layout()
    }

    pub(crate) fn restart(&mut self, source: Arc<dyn SceneSource>) -> Result<()> {
        self.solvers.reset()?;
        self.source = source;
        self.figures = Figures::new(self.source.as_ref());
        self.emitter_states.clear();
        self.step = None;
        self.prepare()
    }

    pub(crate) fn restore(
        &mut self,
        source: Arc<dyn SceneSource>,
        to: u32,
        checkpoint: &Checkpoint,
    ) -> Result<()> {
        self.replace_source(source)?;
        self.solvers.restore(checkpoint)?;
        self.figures = Figures::new(self.source.as_ref());
        let time = self.layout.step_rate.time_of(to) as f32;
        self.figures.pose(self.source.as_ref(), time);
        self.emitter_states = self.emitter_states_at(to);
        self.step = Some(to);
        Ok(())
    }

    pub(crate) fn wake_figures(&mut self, source: Arc<dyn SceneSource>) -> Result<()> {
        self.figures.wake_figures();
        self.replace_source(source)
    }

    fn replace_source(&mut self, source: Arc<dyn SceneSource>) -> Result<()> {
        let emitters_changed = source.emitters() != self.source.emitters();
        self.source = source;
        if emitters_changed {
            self.prepare()
        } else {
            Ok(())
        }
    }

    fn prepare(&mut self) -> Result<()> {
        self.mix = Mix::from_emitters(self.source.emitters());
        let fluid = &mut self.solvers.fluid;
        fluid.set_substances(&self.mix.substances())?;
        for (_, region) in self.regions.drain(..) {
            fluid.remove_region(region);
        }
        for emitter in self.source.emitters() {
            let Some(region_spec) = &emitter.region else {
                continue;
            };
            let Some((_, body)) = self
                .solvers
                .bodies
                .iter()
                .find(|(figure, _)| *figure == region_spec.figure)
            else {
                continue;
            };
            let Some(figure) = self.layout.figures.get(region_spec.figure) else {
                continue;
            };
            let triangles = region::triangles(&figure.model, &region_spec.materials);
            if triangles.is_empty() {
                continue;
            }
            let region = fluid.add_region(*body, &triangles)?;
            self.regions.push((emitter.id, region));
        }
        Ok(())
    }

    fn emitter_states_at(&self, step: u32) -> BTreeMap<u32, EmitterState> {
        self.source
            .emitter_states(step, self.figures.instances())
            .into_iter()
            .map(|emitter_state| (emitter_state.id, emitter_state))
            .collect()
    }

    fn inputs(&self, ends: &BTreeMap<u32, EmitterState>) -> Inputs {
        let mut inputs = Inputs::new().drying(self.layout.drying);
        let instances = self.figures.instances();
        for (figure, body) in &self.solvers.bodies {
            if let Some(instance) = instances.get(*figure) {
                inputs = inputs.body(*body, instance);
            }
        }
        let gravity = self.solvers.fluid.gravity();
        for emitter in self.source.emitters() {
            let Some(end) = ends.get(&emitter.id) else {
                continue;
            };
            let context = EmitterContext {
                substance: self.mix.substance_of(emitter.id),
                gravity,
                region: self.region_of(emitter.id),
            };
            for burst in context.bursts(end) {
                inputs = inputs.emit(burst);
            }
            if let Some(flow) = self
                .emitter_states
                .get(&emitter.id)
                .and_then(|start| context.flow(start, end))
            {
                inputs = inputs.emit(flow);
            }
        }
        inputs
    }

    fn region_of(&self, emitter: u32) -> Option<RegionId> {
        self.regions
            .iter()
            .find(|(key, _)| *key == emitter)
            .map(|(_, region)| *region)
    }
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scene")
            .field("layout", &self.layout)
            .field("step", &self.step)
            .finish_non_exhaustive()
    }
}
