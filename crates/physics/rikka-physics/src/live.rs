use std::sync::Arc;

use glam::Vec3;
use rikka_gpu::Compute;
use rikka_model_core::NodeId;
use rikka_physics_cloth::{Cloth, ClothBuilder, READBACK_LAG, Recorded};
use rikka_physics_tailor::{ClothAsset, Style};
use rikka_renderer_3d::Frame;

use crate::cloth_figures::ClothFigures;
use crate::draping;
use crate::error::Result;
use crate::follow::{Follow, Pace, Step};
use crate::live_inputs::LiveInputs;

const MAX_STEPS_PER_FOLLOW: u32 = READBACK_LAG;
const STEP_EPSILON: f64 = 1.0e-3;

pub struct LiveCloth {
    compute: Compute,
    cloth: Cloth,
    figures: ClothFigures,
    follow: Follow,
    followed: Option<f32>,
    accumulator: f64,
    clock: f64,
}

impl LiveCloth {
    pub fn new(compute: &Compute, cloth: ClothBuilder) -> Result<LiveCloth> {
        Ok(LiveCloth {
            compute: compute.clone(),
            cloth: cloth.build(compute)?,
            figures: ClothFigures::default(),
            follow: Follow::default(),
            followed: None,
            accumulator: 0.0,
            clock: 0.0,
        })
    }

    pub fn add_figure(&mut self, figure: usize, asset: Arc<ClothAsset>) -> Result<()> {
        self.remove_figure(figure)?;
        let id = self.cloth.add_figure(asset)?;
        self.figures.add(figure, id);
        Ok(())
    }

    pub fn remove_figure(&mut self, figure: usize) -> Result<()> {
        let Some(id) = self.figures.remove(figure) else {
            return Ok(());
        };
        self.cloth.remove_figure(id)?;
        Ok(())
    }

    #[must_use]
    pub fn has_figure(&self, figure: usize) -> bool {
        self.figures.id(figure).is_some()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.figures.is_empty()
    }

    pub fn set_style(&mut self, figure: usize, piece: usize, style: Style) -> Result<()> {
        let Some(id) = self.figures.id(figure) else {
            return Ok(());
        };
        self.cloth.set_style(id, piece, style)?;
        Ok(())
    }

    pub fn restart(&mut self) {
        self.followed = None;
    }

    pub fn follow(&mut self, time: f32, pace: Pace, inputs: &LiveInputs<'_>) -> Result<Step> {
        let step = self.follow.step(self.followed, time, pace);
        self.followed = None;
        let mut times = [0.0; MAX_STEPS_PER_FOLLOW as usize];
        let due = match step {
            Step::Seek => {
                self.cloth.reset()?;
                self.accumulator = 0.0;
                self.clock = f64::from(time);
                times[0] = self.clock;
                1
            }
            Step::Advance(delta) => {
                let interval = self.interval();
                let due = self.accumulate(delta) as usize;
                for slot in &mut times[..due] {
                    self.clock += interval;
                    *slot = self.clock;
                }
                due
            }
        };
        if due > 0 {
            self.run(inputs, &times[..due])?;
        }
        self.followed = Some(time);
        Ok(step)
    }

    #[must_use]
    pub fn motion(&self) -> f32 {
        self.cloth.motion()
    }

    #[must_use]
    pub fn steps(&self) -> u64 {
        self.cloth.steps()
    }

    pub fn drapes(
        &self,
        frame: &mut Frame,
        figure: usize,
    ) -> Vec<(NodeId, rikka_renderer_3d::Drape)> {
        let Some(drapes) = self
            .figures
            .id(figure)
            .and_then(|id| self.cloth.drapes(id).ok())
        else {
            return Vec::new();
        };
        draping::to_frame(frame, &drapes)
    }

    pub fn positions(&self, figures: &[usize]) -> Result<Vec<Option<Vec<Vec3>>>> {
        let entries: Vec<_> = figures
            .iter()
            .filter_map(|figure| self.figures.id(*figure))
            .collect();
        if entries.is_empty() {
            return Ok(vec![None; figures.len()]);
        }
        let mut read = self.cloth.read_positions(&entries)?.into_iter();
        Ok(figures
            .iter()
            .map(|figure| self.figures.id(*figure).and_then(|_| read.next()))
            .collect())
    }

    fn run(&mut self, live: &LiveInputs<'_>, times: &[f64]) -> Result<()> {
        let inputs = self.figures.inputs(
            live.wind,
            |figure| {
                live.figures
                    .iter()
                    .rev()
                    .find(|(key, _)| *key == figure)
                    .map(|(_, instance)| *instance)
            },
            &live.grabs,
        );
        let mut batch = self.compute.batch();
        let mut recorded: [Option<Recorded>; MAX_STEPS_PER_FOLLOW as usize] = Default::default();
        for (&time, slot) in times.iter().zip(&mut recorded) {
            *slot = Some(self.cloth.advance_into(&mut batch, time, &inputs)?);
        }
        let ready = batch.submit()?;
        for step in recorded.into_iter().flatten() {
            self.cloth.commit(step, &ready);
        }
        Ok(())
    }

    fn interval(&self) -> f64 {
        f64::from(self.cloth.rate()).recip()
    }

    fn accumulate(&mut self, delta: f32) -> u32 {
        let interval = self.interval();
        self.accumulator += f64::from(delta.max(0.0));
        let due = (self.accumulator / interval + STEP_EPSILON)
            .floor()
            .max(0.0);
        let steps = due.min(f64::from(MAX_STEPS_PER_FOLLOW)) as u32;
        self.accumulator = if due > f64::from(MAX_STEPS_PER_FOLLOW) {
            0.0
        } else {
            self.accumulator - f64::from(steps) * interval
        };
        steps
    }
}

impl std::fmt::Debug for LiveCloth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveCloth")
            .field("figures", &self.figures.len())
            .field("followed", &self.followed)
            .finish_non_exhaustive()
    }
}
