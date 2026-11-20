use glam::Vec3;

use super::cone::Cone;
use super::hull::Hull;
use super::pose::Pose;
use crate::asset::BodyShape;

#[derive(Clone, Debug)]
pub(crate) enum Solid {
    Cone(Cone),
    Hull(Hull),
}

impl Solid {
    pub(crate) fn distance(&self, point: Vec3) -> f32 {
        match self {
            Solid::Cone(cone) => cone.distance(point),
            Solid::Hull(hull) => hull.distance(point),
        }
    }

    pub(crate) fn shrunk(&self, by: f32) -> Solid {
        match self {
            Solid::Cone(cone) => Solid::Cone(cone.shrunk(by)),
            Solid::Hull(hull) => Solid::Hull(hull.shrunk(by)),
        }
    }

    pub(crate) fn thinnest(&self) -> f32 {
        match self {
            Solid::Cone(cone) => cone.thinnest(),
            Solid::Hull(hull) => hull.thinnest(),
        }
    }

    pub(crate) fn moved(&self, pose: Pose) -> Solid {
        match self {
            Solid::Cone(cone) => Solid::Cone(cone.moved(pose)),
            Solid::Hull(hull) => Solid::Hull(hull.moved(pose)),
        }
    }

    pub(crate) fn shape(&self) -> BodyShape {
        match self {
            Solid::Cone(cone) => cone.shape(),
            Solid::Hull(hull) => hull.shape(),
        }
    }

    pub(crate) fn is_solid(&self) -> bool {
        match self {
            Solid::Cone(cone) => cone.thickest() > 0.0,
            Solid::Hull(hull) => hull.is_solid(),
        }
    }

    pub(crate) fn bounds(&self) -> (Vec3, Vec3) {
        match self {
            Solid::Cone(cone) => cone.bounds(),
            Solid::Hull(hull) => hull.bounds(),
        }
    }
}
