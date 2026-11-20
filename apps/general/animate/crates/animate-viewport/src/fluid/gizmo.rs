use animate_fluid::{EmitterId, EmitterSetting, EmitterValue};
use animate_stage::{Aiming, Lens, Stage};
use rikka_app::prelude::*;

use crate::Gesture;
use crate::fluid::dragging::{REACH, SpotDrag, nearest};

const HANDLES: [EmitterSetting; 2] = [EmitterSetting::Launch, EmitterSetting::Target];

#[derive(Debug, Default)]
pub(crate) struct Gizmo {
    drag: Option<SpotDrag>,
    lit: Option<EmitterSetting>,
    snapped: Option<Point>,
}

impl Gizmo {
    pub(crate) fn lit(&self) -> Option<EmitterSetting> {
        self.drag.as_ref().map(SpotDrag::setting).or(self.lit)
    }

    pub(crate) fn snapped(&self) -> Option<Point> {
        self.snapped
    }

    pub(crate) fn press(
        &mut self,
        aiming: &Aiming,
        frame: Rect,
        at: Point,
        modifiers: Modifiers,
    ) -> bool {
        let lens = Lens::new(aiming.view_projection, frame);
        let Some(setting) = handle_at(aiming, &lens, at) else {
            return false;
        };
        let Some(placed) = aiming.placed(setting).copied() else {
            return false;
        };
        let Some(anchor) = aiming.state.spot(setting).map(|spot| spot.anchor.clone()) else {
            return false;
        };
        self.drag = SpotDrag::new(&lens, setting, anchor, placed, at, modifiers);
        self.snapped = None;
        self.drag.is_some()
    }

    pub(crate) fn travel(&mut self, stage: &mut Stage, frame_at: u32, rect: Rect, at: Point) {
        let (Some(aiming), Some(drag)) = (stage.aiming(), &mut self.drag) else {
            return;
        };
        let lens = Lens::new(aiming.view_projection, rect);
        let offset = drag.advance(&lens, at);
        self.snapped = drag
            .attaching()
            .then(|| nearest(&lens, &aiming.joints, at))
            .flatten()
            .and_then(|joint| lens.project(joint.world));
        let (emitter, setting) = (aiming.emitter, drag.setting());
        stage.drafts_mut().pour.set(
            emitter,
            frame_at,
            setting,
            EmitterValue::Three(offset.to_array()),
        );
    }

    pub(crate) fn release(
        &mut self,
        aiming: Option<&Aiming>,
        rect: Rect,
        at: Point,
    ) -> Option<Gesture> {
        self.snapped = None;
        let drag = self.drag.take()?;
        let aiming = aiming?;
        let lens = Lens::new(aiming.view_projection, rect);
        let setting = drag.setting();
        Some(match drag.attaching() {
            true => {
                let (anchor, offset) = drag.landing(&lens, &aiming.joints, at);
                Gesture::Attach {
                    setting,
                    anchor,
                    offset,
                }
            }
            false => Gesture::Pour(
                aiming.emitter,
                setting,
                EmitterValue::Three(drag.offset().to_array()),
            ),
        })
    }

    pub(crate) fn cancel(&mut self, stage: &mut Stage, emitter: EmitterId) -> bool {
        self.snapped = None;
        let Some(drag) = self.drag.take() else {
            return false;
        };
        stage.drafts_mut().pour.clear(emitter, drag.setting());
        true
    }

    pub(crate) fn hover(&mut self, aiming: Option<&Aiming>, rect: Rect, at: Option<Point>) -> bool {
        let highlighted = aiming.zip(at).and_then(|(aiming, at)| {
            handle_at(aiming, &Lens::new(aiming.view_projection, rect), at)
        });
        let changed = self.lit != highlighted;
        self.lit = highlighted;
        changed
    }
}

fn handle_at(aiming: &Aiming, lens: &Lens, at: Point) -> Option<EmitterSetting> {
    HANDLES
        .into_iter()
        .filter_map(|setting| {
            let screen_point = lens.project(aiming.placed(setting)?.world)?;
            Some((setting, screen_point.distance(at)))
        })
        .filter(|(_, away)| *away <= REACH)
        .min_by(|one, two| one.1.total_cmp(&two.1))
        .map(|(setting, _)| setting)
}
