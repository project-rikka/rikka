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
use crate::live_inputs::LivePose;

const MAX_STEPS_PER_FOLLOW: usize = READBACK_LAG as usize;
const STEP_EPSILON: f64 = 1.0e-3;

#[derive(Clone, Copy, Debug, Default)]
struct Due {
    clock: f64,
    posed: f32,
}

pub struct LiveCloth {
    compute: Compute,
    cloth: Cloth,
    figures: ClothFigures,
    follow: Follow,
    followed: Option<f32>,
    posed: Option<f32>,
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
            posed: None,
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
        self.posed = None;
    }

    pub fn follow(&mut self, time: f32, pace: Pace, pose: &mut impl LivePose) -> Result<Step> {
        let step = self.follow.step(self.followed, time, pace);
        self.followed = None;
        let mut dues = [Due::default(); MAX_STEPS_PER_FOLLOW];
        let due = match step {
            Step::Seek => {
                self.cloth.reset()?;
                self.accumulator = 0.0;
                self.clock = f64::from(time);
                dues[0] = Due {
                    clock: self.clock,
                    posed: time,
                };
                1
            }
            Step::Advance(delta) => {
                let interval = self.interval();
                let due = self.accumulate(delta);
                for (at, slot) in dues[..due].iter_mut().enumerate() {
                    self.clock += interval;
                    let behind = self.accumulator + (due - 1 - at) as f64 * interval;
                    let posed = match pace {
                        Pace::Playing => time - behind.max(0.0) as f32,
                        Pace::Stopped { .. } => time,
                    };
                    *slot = Due {
                        clock: self.clock,
                        posed,
                    };
                }
                due
            }
        };
        if due > 0 {
            self.run(pose, &dues[..due])?;
            self.posed = Some(dues[due - 1].posed);
        }
        self.followed = Some(time);
        Ok(step)
    }

    #[must_use]
    pub fn posed(&self) -> Option<f32> {
        self.posed
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

    fn run(&mut self, pose: &mut impl LivePose, dues: &[Due]) -> Result<()> {
        let mut batch = self.compute.batch();
        let mut recorded: [Option<Recorded>; MAX_STEPS_PER_FOLLOW] = Default::default();
        for (due, slot) in dues.iter().zip(&mut recorded) {
            let live = pose.pose(due.posed);
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
            *slot = Some(self.cloth.advance_into(&mut batch, due.clock, &inputs)?);
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

    fn accumulate(&mut self, delta: f32) -> usize {
        let interval = self.interval();
        self.accumulator += f64::from(delta.max(0.0));
        let due = (self.accumulator / interval + STEP_EPSILON)
            .floor()
            .max(0.0);
        let steps = due.min(MAX_STEPS_PER_FOLLOW as f64) as usize;
        self.accumulator = if due > MAX_STEPS_PER_FOLLOW as f64 {
            0.0
        } else {
            self.accumulator - steps as f64 * interval
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
