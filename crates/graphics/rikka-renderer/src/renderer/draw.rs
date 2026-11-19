use std::ops::Deref;

#[cfg(unix)]
use ash::vk;
use rikka_types::Rect;
use smallvec::SmallVec;

use crate::renderer::Renderer;
use crate::renderer::canvas::{Onto, Painted, Plan};
use crate::renderer::history::{TargetState, Viewing};
use crate::renderer::painter::PaintState;
use crate::scene::Scene;
use rikka_gpu::error::Result;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Damage {
    rects: SmallVec<[Rect; 8]>,
}

impl Damage {
    pub fn into_vec(self) -> Vec<Rect> {
        self.rects.into_vec()
    }
}

impl Deref for Damage {
    type Target = [Rect];

    fn deref(&self) -> &[Rect] {
        &self.rects
    }
}

impl FromIterator<Rect> for Damage {
    fn from_iter<I: IntoIterator<Item = Rect>>(rects: I) -> Self {
        Self {
            rects: rects.into_iter().collect(),
        }
    }
}

impl<'a> IntoIterator for &'a Damage {
    type Item = &'a Rect;
    type IntoIter = std::slice::Iter<'a, Rect>;

    fn into_iter(self) -> Self::IntoIter {
        self.rects.iter()
    }
}

#[derive(Debug, Default)]
pub struct RenderOutput {
    pub damage: Damage,
    pub needs_full_redraw: bool,
}

impl RenderOutput {
    pub fn rendered(&self) -> bool {
        !self.damage.is_empty()
    }

    pub(crate) fn deferred() -> Self {
        Self {
            damage: Damage::default(),
            needs_full_redraw: true,
        }
    }
}

#[derive(Debug, Default)]
#[must_use = "a recorded frame is only seen once it is presented"]
pub struct Recorded {
    output: RenderOutput,
    image: Option<u32>,
}

impl Recorded {
    pub fn is_presentable(&self) -> bool {
        self.image.is_some()
    }

    pub fn deferred() -> Self {
        Self {
            output: RenderOutput::deferred(),
            image: None,
        }
    }

    pub fn new(output: RenderOutput, image: u32) -> Self {
        Self {
            output,
            image: Some(image),
        }
    }

    pub fn image(&self) -> Option<u32> {
        self.image
    }

    pub fn into_output(self) -> RenderOutput {
        self.output
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameStart {
    Idle,
    Deferred,
    Draw,
}

pub trait Destination: Onto {}

impl<T: Onto> Destination for T {}

impl Renderer {
    pub fn draw<D: Destination>(&mut self, scene: &mut Scene, onto: D) -> Result<D::Output> {
        self.paint(scene, onto)
    }

    pub(crate) fn start_plan(&mut self, scene: &mut Scene, plan: Plan) -> Result<FrameStart> {
        self.reclaim();
        scene.settle_order();
        if !self.targets.is_behind(plan.target) && self.scene_is_settled(scene, &plan) {
            return Ok(FrameStart::Idle);
        }
        match self.needs_full_frame(scene, &plan) {
            true => Ok(FrameStart::Deferred),
            false => Ok(FrameStart::Draw),
        }
    }

    pub(crate) fn paint<D: Onto>(&mut self, scene: &mut Scene, mut onto: D) -> Result<D::Output> {
        self.reclaim();
        scene.settle_order();
        let canvas = onto.canvas();
        let plan = canvas.plan;
        if canvas.underlay {
            scene.damage_whole_for(plan.viewer);
        }
        if !onto.has_copy_work()
            && self.targets.up_to_date(plan.target, plan.buffer)
            && self.scene_is_settled(scene, &plan)
        {
            return Ok(onto.undrawn(RenderOutput::default()));
        }
        if self.needs_full_frame(scene, &plan) {
            return Ok(onto.undrawn(RenderOutput::deferred()));
        }
        #[cfg(unix)]
        let canvas = match onto.released_by() {
            Some(fence) => match self.gpu.wait_on_sync_fd(fence)? {
                Some(semaphore) => {
                    canvas.waits_on(semaphore, vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                }
                None => canvas,
            },
            None => canvas,
        };
        let copy = match onto.copies_out()? {
            Some(out) => {
                Some(out.reserve(self.gpu.device(), self.gpu.completed_value(), canvas.texels)?)
            }
            None => None,
        };
        let output = self.painter.frame(
            &mut self.gpu,
            PaintState {
                targets: &mut self.targets,
                sources: &mut self.sources,
                colors: &self.colors,
            },
            scene,
            &canvas,
            copy,
        )?;
        let submitted = self.submitted_frame();
        onto.drawn(Painted {
            output,
            submitted,
            gpu: &mut self.gpu,
        })
    }

    fn scene_is_settled(&self, scene: &Scene, plan: &Plan) -> bool {
        let viewing = Viewing {
            scene: scene.id(),
            viewer: plan.viewer,
        };
        !scene.has_damage_for(plan.viewer)
            && !self.sources.has_pending()
            && !self.extension.has_pending()
            && self.targets.views(plan.target, viewing)
            && !self.targets.needs_full(plan)
    }

    fn needs_full_frame(&mut self, scene: &mut Scene, plan: &Plan) -> bool {
        if !plan.composes(scene) {
            return false;
        }
        if !self.targets.needs_full(plan)
            && self
                .targets
                .get(plan.target)
                .is_some_and(TargetState::holds_scene)
        {
            return false;
        }
        scene.is_immediate() && !scene.damage_covers(plan.viewer, plan.viewport())
    }
}
