use glam::Vec3;

use super::pose::Pose;
use crate::asset::BodyShape;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Cone {
    pub(crate) a: Vec3,
    pub(crate) b: Vec3,
    pub(crate) radius_a: f32,
    pub(crate) radius_b: f32,
}

impl Cone {
    pub(crate) fn shape(&self) -> BodyShape {
        BodyShape::Tapered {
            ends: [self.a, self.b],
            radii: [self.radius_a, self.radius_b],
        }
    }

    pub(crate) fn distance(&self, point: Vec3) -> f32 {
        self.shape().distance(point)
    }

    pub(crate) fn shrunk(&self, by: f32) -> Cone {
        Cone {
            radius_a: self.radius_a - by,
            radius_b: self.radius_b - by,
            ..*self
        }
    }

    pub(crate) fn thinnest(&self) -> f32 {
        self.radius_a.min(self.radius_b)
    }

    pub(crate) fn thickest(&self) -> f32 {
        self.radius_a.max(self.radius_b)
    }

    pub(crate) fn moved(&self, pose: Pose) -> Cone {
        Cone {
            a: pose.apply(self.a),
            b: pose.apply(self.b),
            ..*self
        }
    }

    pub(crate) fn bounds(&self) -> (Vec3, Vec3) {
        let reach = Vec3::splat(self.thickest());
        (self.a.min(self.b) - reach, self.a.max(self.b) + reach)
    }
}
