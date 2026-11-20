use std::sync::Arc;

use rikka_gpu::Compute;

use super::handle::{Bake, ShowStatus};
use super::replay::SceneReplay;
use crate::error::Result;
use crate::layout::Layout;
use crate::scene_frame::SceneFrame;

const BUSY_REUSE_STEPS: u32 = 2;

#[derive(Default)]
pub struct BakeView {
    replay: Option<SceneReplay>,
    failed_layout: Option<Arc<Layout>>,
    shown: Option<(u64, u32)>,
    busy_miss: bool,
}

impl BakeView {
    pub fn frame(
        &mut self,
        bake: &Bake,
        compute: &Compute,
        step: u32,
    ) -> Result<Option<SceneFrame>> {
        let status = bake.watch(step);
        self.busy_miss = false;
        let Some(layout) = status.layout.clone() else {
            self.replay = None;
            self.shown = None;
            return Ok(None);
        };
        if self
            .failed_layout
            .as_ref()
            .is_some_and(|failed_layout| Arc::ptr_eq(failed_layout, &layout))
        {
            return Ok(None);
        }
        let replayed = self
            .replay
            .as_ref()
            .is_some_and(|replay| Arc::ptr_eq(&replay.layout, &layout));
        if !replayed {
            self.replay = None;
            self.shown = None;
            let replay = SceneReplay::build(Arc::clone(&layout), compute).inspect_err(|_| {
                self.failed_layout = Some(Arc::clone(&layout));
            })?;
            self.replay = Some(replay);
        }
        let Some(replay) = self.replay.as_mut() else {
            return Ok(None);
        };
        let target = (status.epoch, step);
        if self.shown != Some(target) {
            if !status.showable {
                self.shown = None;
                return Ok(None);
            }
            match bake.show(step, replay)? {
                ShowStatus::Shown => self.shown = Some(target),
                ShowStatus::Missing => {
                    self.shown = None;
                    return Ok(None);
                }
                ShowStatus::Busy => {
                    self.busy_miss = true;
                    let near = self.shown.is_some_and(|(epoch, shown)| {
                        epoch == status.epoch && shown.abs_diff(step) <= BUSY_REUSE_STEPS
                    });
                    if !near {
                        return Ok(None);
                    }
                }
            }
        }
        Ok(Some(replay.frame()))
    }

    #[must_use]
    pub fn is_stale(&self, bake: &Bake, step: u32) -> bool {
        if self.busy_miss {
            return true;
        }
        let status = bake.status(step);
        let showing = self.shown == Some((status.epoch, step));
        let replayed = match (&self.replay, &status.layout) {
            (Some(replay), Some(layout)) => Arc::ptr_eq(&replay.layout, layout),
            (None, Some(layout)) => self
                .failed_layout
                .as_ref()
                .is_some_and(|failed_layout| Arc::ptr_eq(failed_layout, layout)),
            (None, None) => true,
            (Some(_), None) => false,
        };
        !replayed || (!showing && (status.showable || self.shown.is_some()))
    }
}

impl std::fmt::Debug for BakeView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BakeView")
            .field("shown", &self.shown)
            .finish_non_exhaustive()
    }
}
