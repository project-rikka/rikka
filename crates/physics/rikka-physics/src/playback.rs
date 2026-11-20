use std::sync::Arc;

use rikka_gpu::Compute;

use crate::bake::{Bake, BakeView, Progress};
use crate::error::{Error, Result};
use crate::scene::Scene;
use crate::scene_frame::SceneFrame;
use crate::scene_source::{SceneSource, has_emitters};

#[derive(Default)]
pub enum Playback {
    #[default]
    Off,
    Baked {
        bake: Bake,
        display: Box<BakeView>,
    },
    Simulated(Option<Box<Scene>>),
}

impl Playback {
    pub fn baked(bake: Bake) -> Playback {
        Playback::Baked {
            bake,
            display: Box::default(),
        }
    }

    pub fn simulated() -> Playback {
        Playback::Simulated(None)
    }

    pub fn frame(
        &mut self,
        compute: &Compute,
        source: &Arc<dyn SceneSource>,
        step: u32,
    ) -> Result<Option<SceneFrame>> {
        match self {
            Playback::Off => Ok(None),
            Playback::Baked { bake, display } => {
                bake.set_compute(compute);
                display.frame(bake, compute, step)
            }
            Playback::Simulated(current) => {
                if !has_emitters(source.as_ref()) {
                    return Ok(None);
                }
                let needs_rebuild = current
                    .as_ref()
                    .is_none_or(|scene| scene.step().is_some_and(|at| at > step));
                if needs_rebuild {
                    *current = None;
                    *current = Some(Box::new(Scene::build(compute, Arc::clone(source))?));
                }
                let Some(scene) = current.as_mut() else {
                    return Ok(None);
                };
                while scene.step().is_none_or(|at| at < step) {
                    scene.advance()?;
                }
                Ok(Some(scene.frame()))
            }
        }
    }

    #[must_use]
    pub fn is_stale(&self, step: u32) -> bool {
        match self {
            Playback::Baked { bake, display } => display.is_stale(bake, step),
            Playback::Off | Playback::Simulated(_) => false,
        }
    }

    pub fn set_source(&self, source: &Arc<dyn SceneSource>, since: Option<u32>) {
        if let Playback::Baked { bake, .. } = self {
            bake.set_source(Arc::clone(source), since);
        }
    }

    pub fn set_target_step(&self, step: u32) {
        if let Playback::Baked { bake, .. } = self {
            bake.set_target_step(step);
        }
    }

    pub fn pause(&self, paused: bool) {
        if let Playback::Baked { bake, .. } = self {
            bake.pause(paused);
        }
    }

    pub fn recompute(&self) {
        if let Playback::Baked { bake, .. } = self {
            bake.recompute();
        }
    }

    #[must_use]
    pub fn progress(&self) -> Option<Progress> {
        match self {
            Playback::Baked { bake, .. } => Some(bake.progress()),
            Playback::Off | Playback::Simulated(_) => None,
        }
    }

    pub fn take_error(&mut self) -> Option<Error> {
        match self {
            Playback::Baked { bake, .. } => bake.take_error(),
            Playback::Off | Playback::Simulated(_) => None,
        }
    }
}

impl std::fmt::Debug for Playback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            Playback::Off => "Off",
            Playback::Baked { .. } => "Baked",
            Playback::Simulated(_) => "Simulated",
        };
        f.write_str(kind)
    }
}
