use animate_body::Side;
use animate_hold::{Hold, HoldTarget, Holder};
use animate_stage::Spot;
use rikka_app::prelude::*;

use crate::easy::scene::Scene;

const HAND_SHARE: f32 = 0.6;
const WIDENING: f32 = 1.2;
const NARROWEST: f32 = 0.3;
const WIDEST: f32 = 3.0;

#[derive(Debug)]
pub(super) struct Grasping {
    side: Side,
    pointer: Point,
    reach: f32,
    natural: f32,
    spot: Option<Spot>,
}

impl Grasping {
    pub(super) fn new(scene: &Scene, side: Side, at: Point) -> Option<Self> {
        let natural = scene.body.pinch(side)?.local.length() * HAND_SHARE;
        Some(Self {
            side,
            pointer: at,
            reach: natural,
            natural,
            spot: None,
        })
    }

    pub(super) fn side(&self) -> Side {
        self.side
    }

    pub(super) fn pointer(&self) -> Point {
        self.pointer
    }

    pub(super) fn spot(&self) -> Option<&Spot> {
        self.spot.as_ref()
    }

    pub(super) fn reach(&self) -> f32 {
        self.reach
    }

    pub(super) fn travel(&mut self, scene: &Scene, at: Point) {
        self.pointer = at;
        self.spot = scene
            .cloth_snapshot
            .as_ref()
            .and_then(|cloth_snapshot| cloth_snapshot.spot_at(&scene.lens, at));
    }

    pub(super) fn widen(&mut self, notches: f32) {
        self.reach = (self.reach * WIDENING.powf(notches))
            .clamp(self.natural * NARROWEST, self.natural * WIDEST);
    }

    pub(super) fn hold(&self, scene: &Scene) -> Option<Hold> {
        let spot = self.spot.as_ref()?;
        if scene
            .cloth_snapshot
            .as_ref()?
            .patch(spot, self.reach)
            .is_empty()
        {
            return None;
        }
        let target = HoldTarget {
            figure: (spot.figure != scene.id).then(|| spot.name.clone()),
            at: spot.rest,
            reach: self.reach,
        };
        Some(Hold::new(Holder::Hand(self.side), target, scene.frame))
    }
}
