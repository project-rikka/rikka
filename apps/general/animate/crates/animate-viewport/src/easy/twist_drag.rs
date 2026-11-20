use rikka_app::prelude::*;
use rikka_model::glam::Vec3;

use crate::easy::scene::Scene;
use crate::easy::style::TWIST_PACE;

const FACING: f32 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TwistDrag {
    press: f32,
    sign: f32,
}

impl TwistDrag {
    pub(super) fn new(scene: &Scene, axis: Vec3, point: Vec3, press: Point) -> Self {
        let lens = &scene.lens;
        let ahead = lens.ahead(point);
        let (right, _, _) = lens.sideways(point);
        let near = axis.cross(-ahead).dot(right);
        let facing = match near.abs() > FACING {
            true => near,
            false => axis.dot(ahead),
        };
        Self {
            press: press.x,
            sign: match facing < 0.0 {
                true => -1.0,
                false => 1.0,
            },
        }
    }

    pub(super) fn advance(&self, at: Point) -> f32 {
        (at.x - self.press) * self.sign / TWIST_PACE
    }
}
