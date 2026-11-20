use rikka_physics_math::{Transform, Vec3};

use crate::triangulated::triangle::TriangleFeature;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContactFeature {
    Triangle(TriangleFeature),
    HullFace,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct LocalPoint {
    pub(crate) point: Vec3,
    pub(crate) separation: f32,
    pub(crate) id: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LocalManifold {
    pub(crate) normal: Vec3,
    pub(crate) feature: ContactFeature,
    pub(crate) squared_distance: f32,
}

#[derive(Debug)]
pub(crate) struct PointSink<'a> {
    points: &'a mut Vec<LocalPoint>,
    start: usize,
}

impl<'a> PointSink<'a> {
    #[inline]
    pub(crate) fn new(points: &'a mut Vec<LocalPoint>) -> Self {
        let start = points.len();
        Self { points, start }
    }

    #[inline]
    pub(crate) fn push(&mut self, point: Vec3, separation: f32, id: u32) {
        self.points.push(LocalPoint {
            point,
            separation,
            id,
        });
    }

    #[inline]
    pub(crate) fn count(&self) -> usize {
        self.points.len() - self.start
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.points.truncate(self.start);
    }

    #[inline]
    pub(crate) fn min_separation(&self) -> f32 {
        self.points[self.start..]
            .iter()
            .fold(f32::MAX, |min, point| min.min(point.separation))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PlacedTriangle {
    pub(crate) local: [Vec3; 3],
    pub(crate) vertices: [Vec3; 3],
    pub(crate) transform: Transform,
}
