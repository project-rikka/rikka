use animate_stage::{Blowing, Lens, Stage, Vane};
use animate_wind::WindSetting;
use rikka_app::prelude::*;

use crate::Gesture;

const REACH: f32 = 12.0;

#[derive(Debug, Default)]
pub(crate) struct Grip {
    grab: bool,
    turned: Option<f32>,
    lit: bool,
}

impl Grip {
    pub(crate) fn lit(&self) -> bool {
        self.grab || self.lit
    }

    pub(crate) fn press(&mut self, vane: &Vane, frame: Rect, at: Point) -> bool {
        self.grab = on_tip(vane, frame, at);
        self.turned = None;
        self.grab
    }

    pub(crate) fn travel(&mut self, stage: &mut Stage, frame_at: u32, rect: Rect, at: Point) {
        let Some(vane) = stage.vane().copied().filter(|_| self.grab) else {
            return;
        };
        let Some(yaw) = vane.yaw_at(&Lens::new(vane.view_projection, rect), at) else {
            return;
        };
        self.turned = Some(yaw);
        stage
            .drafts_mut()
            .wind
            .set(Blowing, frame_at, WindSetting::Yaw, yaw);
    }

    pub(crate) fn release(&mut self) -> Option<Gesture> {
        let grabbed = std::mem::take(&mut self.grab);
        let pending_yaw = self.turned.take();
        pending_yaw
            .filter(|_| grabbed)
            .map(|yaw| Gesture::Wind(WindSetting::Yaw, yaw))
    }

    pub(crate) fn cancel(&mut self, stage: &mut Stage) -> bool {
        self.turned = None;
        if !std::mem::take(&mut self.grab) {
            return false;
        }
        stage.drafts_mut().wind.clear(Blowing, WindSetting::Yaw);
        true
    }

    pub(crate) fn hover(&mut self, vane: Option<&Vane>, rect: Rect, at: Option<Point>) -> bool {
        let highlighted = vane
            .zip(at)
            .is_some_and(|(vane, at)| on_tip(vane, rect, at));
        let changed = self.lit != highlighted;
        self.lit = highlighted;
        changed
    }
}

fn on_tip(vane: &Vane, frame: Rect, at: Point) -> bool {
    Lens::new(vane.view_projection, frame)
        .project(vane.tip())
        .is_some_and(|tip| tip.distance(at) <= REACH)
}
