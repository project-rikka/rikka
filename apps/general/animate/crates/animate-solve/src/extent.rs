use std::f32::consts::PI;

use animate_body::{Body, Dof, Limb, muscle_of, muscles};
use rikka_model_core::glam::Vec3;

use crate::tuning::REACH_SLACK;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Extent {
    pub(crate) near: f32,
    pub(crate) far: f32,
}

impl Extent {
    pub(crate) fn measure(body: &Body, limb: Limb, limits: bool) -> Self {
        let [_, lower, end] = limb.chain();
        let upper = body.offset(lower).length();
        let fore = body.offset(end).length();
        let folded = muscle_of(lower, Dof::Bend)
            .map_or(PI, |index| muscles()[index].angles(limits).1)
            .min(PI);
        let near = (upper * upper + fore * fore + 2.0 * upper * fore * folded.cos())
            .max(0.0)
            .sqrt();
        Self {
            near: near / REACH_SLACK,
            far: upper + fore,
        }
    }

    pub(crate) fn keep(&self, centre: Vec3, point: Vec3) -> Vec3 {
        let apart = point - centre;
        let distance = apart.length();
        match apart.try_normalize() {
            Some(toward) if distance > self.far => centre + toward * self.far,
            Some(toward) if distance < self.near => centre + toward * self.near,
            _ => point,
        }
    }
}
