mod capsules;
mod cuboid;
mod spheres;

pub use capsules::collide_batch_capsules;
pub use cuboid::collide_batch_cuboid_sphere;
use rikka_physics_math::{Mask, QuatW, Transform, Vec3, Vec3W, Wide};
pub use spheres::{collide_batch_capsule_sphere, collide_batch_spheres};

use super::clip::FeaturePair;
use super::round::point;
use crate::convex::capsule::Capsule;
use crate::convex::cuboid::Cuboid;
use crate::convex::sphere::Sphere;
use crate::{Manifold, Tolerance};

#[derive(Clone, Copy, Debug)]
pub struct TransformLanes<W: Wide> {
    pub p: Vec3W<W>,
    pub q: QuatW<W>,
}

impl<W: Wide> TransformLanes<W> {
    #[inline(always)]
    pub fn from_fn(mut f: impl FnMut(usize) -> Transform) -> Self {
        let mut transforms = [Transform::IDENTITY; 8];
        for (lane, slot) in transforms.iter_mut().enumerate().take(W::LANES) {
            *slot = f(lane);
        }
        Self {
            p: Vec3W::from_fn(|lane| transforms[lane].p),
            q: QuatW::from_fn(|lane| transforms[lane].q),
        }
    }

    #[inline(always)]
    pub fn apply(&self, point: Vec3W<W>) -> Vec3W<W> {
        self.q.rotate(point) + self.p
    }

    #[inline(always)]
    pub fn apply_inverse(&self, point: Vec3W<W>) -> Vec3W<W> {
        self.q.rotate_inverse(point - self.p)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SphereLanes<W: Wide> {
    pub center: Vec3W<W>,
    pub radius: W,
}

impl<W: Wide> SphereLanes<W> {
    #[inline(always)]
    pub fn from_fn(mut f: impl FnMut(usize) -> Sphere) -> Self {
        let mut spheres = [Sphere::with_center(Vec3::ZERO, 0.0); 8];
        for (lane, slot) in spheres.iter_mut().enumerate().take(W::LANES) {
            *slot = f(lane);
        }
        Self {
            center: Vec3W::from_fn(|lane| spheres[lane].center),
            radius: W::from_fn(|lane| spheres[lane].radius),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CapsuleLanes<W: Wide> {
    pub center1: Vec3W<W>,
    pub center2: Vec3W<W>,
    pub radius1: W,
    pub radius2: W,
}

impl<W: Wide> CapsuleLanes<W> {
    #[inline(always)]
    pub fn from_fn(mut f: impl FnMut(usize) -> Capsule) -> Self {
        let mut capsules = [Capsule::segment(Vec3::ZERO, Vec3::X, 1.0); 8];
        for (lane, slot) in capsules.iter_mut().enumerate().take(W::LANES) {
            *slot = f(lane);
        }
        Self {
            center1: Vec3W::from_fn(|lane| capsules[lane].center1),
            center2: Vec3W::from_fn(|lane| capsules[lane].center2),
            radius1: W::from_fn(|lane| capsules[lane].radius1),
            radius2: W::from_fn(|lane| capsules[lane].radius2),
        }
    }

    #[inline(always)]
    fn tapered(&self) -> W::Mask {
        !self.radius1.simd_eq(self.radius2)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CuboidLanes<W: Wide> {
    pub transform: TransformLanes<W>,
    pub half_extents: Vec3W<W>,
    pub radius: W,
}

impl<W: Wide> CuboidLanes<W> {
    #[inline(always)]
    pub fn from_fn(mut f: impl FnMut(usize) -> Cuboid) -> Self {
        let mut cuboids = [Cuboid::new(Vec3::ONE); 8];
        for (lane, slot) in cuboids.iter_mut().enumerate().take(W::LANES) {
            *slot = f(lane);
        }
        Self {
            transform: TransformLanes::from_fn(|lane| cuboids[lane].transform),
            half_extents: Vec3W::from_fn(|lane| cuboids[lane].half_extents),
            radius: W::from_fn(|lane| cuboids[lane].radius),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BatchManifold<W: Wide> {
    pub normal: Vec3W<W>,
    pub point: Vec3W<W>,
    pub separation: W,
    pub touching: W::Mask,
    pub fallback: W::Mask,
}

impl<W: Wide> BatchManifold<W> {
    #[inline(always)]
    pub fn manifold(&self, lane: usize) -> Option<Manifold> {
        if self.fallback.lane(lane) {
            return None;
        }
        let mut manifold = Manifold::default();
        if self.touching.lane(lane) {
            manifold.normal = self.normal.lane(lane);
            manifold.push(point(
                self.point.lane(lane),
                self.separation.lane(lane),
                FeaturePair::SINGLE,
            ));
        }
        Some(manifold)
    }
}

#[inline(always)]
fn linear_slop<W: Wide>(speculative: W) -> W {
    speculative
        * W::splat(Tolerance::default().linear_slop() / Tolerance::default().speculative_distance())
}
