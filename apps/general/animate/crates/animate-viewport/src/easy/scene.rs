use std::sync::Arc;

use animate_body::{Body, Expression, Faces, Limb, MorphWeights, Side, Worlds};
use animate_cast::FigureId;
use animate_hold::{GripPoint, Holder};
use animate_motion::{GazeMode, MotionBake, Part, PoseSample, Take};
use animate_solve::{Finger, Handle, Pin};
use animate_stage::{ClothSnapshot, Lens};
use rikka_app::prelude::*;
use rikka_model::glam::Vec3;

use crate::easy::face;
use crate::easy::part::{Marker, holds_hand};
use crate::easy::style::{CONTROL_REACH, RING, RING_REACH, radius};

#[derive(Clone, Debug)]
pub(crate) struct Scene {
    pub id: FigureId,
    pub body: Arc<Body>,
    pub take: Arc<Take>,
    pub motion_bake: Option<Arc<MotionBake>>,
    pub lens: Lens,
    pub shown: Worlds,
    pub frame: u32,
    pub pose_sample: PoseSample,
    pub part: Option<Part>,
    pub paths: bool,
    pub cloth_snapshot: Option<Arc<ClothSnapshot>>,
    pub graspable: bool,
    pub grips: Vec<GripPoint>,
    pub faces: Arc<Faces>,
    pub morphs: MorphWeights,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Control {
    pub(super) marker: Marker,
    pub(super) point: Vec3,
    pub(super) at: Point,
    pub(super) depth: f32,
    pub(super) pinned: bool,
}

pub(super) struct Sight {
    pub(super) controls: Vec<Control>,
    pub(super) rings: Vec<(Side, Point)>,
    pub(super) gaze: Option<(Vec3, Point)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pointed {
    Marker(Marker),
    Ring(Side),
}

impl Sight {
    pub(super) fn control_at(&self, at: Point) -> Option<Control> {
        self.controls
            .iter()
            .filter(|control| at.distance(control.at) <= radius(control.marker) + CONTROL_REACH)
            .min_by(|one, other| {
                at.distance(one.at)
                    .total_cmp(&at.distance(other.at))
                    .then(one.depth.total_cmp(&other.depth))
            })
            .copied()
    }

    pub(super) fn find(&self, marker: Marker) -> Option<&Control> {
        self.controls
            .iter()
            .find(|control| control.marker == marker)
    }

    pub(super) fn ring_at(&self, at: Point) -> Option<Side> {
        let off = |centre: Point| (at.distance(centre) - RING).abs();
        self.rings
            .iter()
            .filter(|(_, centre)| off(*centre) <= RING_REACH)
            .min_by(|one, other| off(one.1).total_cmp(&off(other.1)))
            .map(|(side, _)| *side)
    }

    pub(super) fn ring(&self, side: Side) -> Option<Point> {
        self.rings
            .iter()
            .find(|(candidate, _)| *candidate == side)
            .map(|(_, centre)| *centre)
    }

    pub(super) fn pointed_at(&self, at: Point) -> Option<Pointed> {
        match self.control_at(at) {
            Some(control) => Some(Pointed::Marker(control.marker)),
            None => self.ring_at(at).map(Pointed::Ring),
        }
    }
}

impl Scene {
    pub(super) fn opened(&self) -> f32 {
        let mut weights = self.take.morph_weights(self.frame as f32);
        weights.overlay(&self.morphs);
        self.faces.strength(&weights, Expression::Aa)
    }

    pub(super) fn limits(&self) -> bool {
        self.take
            .human
            .as_ref()
            .is_none_or(|human| human.settings.limits)
    }

    pub(super) fn paths(&self) -> bool {
        self.paths
    }
    pub(super) fn grip_of(&self, side: Side) -> Option<&GripPoint> {
        self.grips
            .iter()
            .find(|grip| grip.holder == Holder::Hand(side))
    }

    pub(super) fn pinned(&self, limb: Limb) -> bool {
        self.pose_sample.pins[limb.index()].pin
    }

    pub(super) fn pins(&self) -> Vec<Pin> {
        let scale = self.body.scale();
        Limb::ALL
            .into_iter()
            .filter(|limb| self.pinned(*limb))
            .map(|limb| {
                let goal = self.pose_sample.pins[limb.index()].goal;
                Pin {
                    limb,
                    position: goal.position * scale,
                    rotation: goal.rotation,
                }
            })
            .collect()
    }

    pub(super) fn gaze_target(&self) -> Option<Vec3> {
        let gaze = self.pose_sample.gaze;
        (gaze.mode == GazeMode::Point).then(|| gaze.target * self.body.scale())
    }

    fn shows_fingers(&self, side: Side) -> bool {
        self.part.is_some_and(|part| holds_hand(part, side))
    }

    pub(super) fn projection(&self) -> Sight {
        let body = &self.body;
        let handles = Handle::ALL
            .into_iter()
            .filter(|handle| handle.fits(body))
            .map(|handle| (Marker::Handle(handle), handle.point(body, &self.shown)));
        let fingers = Side::BOTH
            .into_iter()
            .filter(|side| self.shows_fingers(*side))
            .flat_map(Finger::all)
            .filter(|finger| finger.fits(body))
            .map(|finger| (Marker::Finger(finger), finger.tip(body, &self.shown)));
        let mut controls: Vec<Control> = handles
            .chain(fingers)
            .chain(face::markers(body, &self.faces, &self.shown))
            .filter_map(|(marker, point)| {
                Some(Control {
                    marker,
                    point,
                    at: self.lens.project(point)?,
                    depth: self.lens.depth(point),
                    pinned: marker
                        .handle()
                        .and_then(|handle| handle.end())
                        .is_some_and(|limb| self.pinned(limb)),
                })
            })
            .collect();
        controls.sort_by(|one, other| other.depth.total_cmp(&one.depth));
        let rings = Side::BOTH
            .into_iter()
            .filter(|side| self.graspable && body.pinch(*side).is_some())
            .filter_map(|side| {
                let hand = controls
                    .iter()
                    .find(|control| control.marker == Marker::Handle(Handle::Hand(side)))?;
                Some((side, hand.at))
            })
            .collect();
        let gaze = self
            .gaze_target()
            .and_then(|target| Some((target, self.lens.project(target)?)));
        Sight {
            controls,
            rings,
            gaze,
        }
    }
}
