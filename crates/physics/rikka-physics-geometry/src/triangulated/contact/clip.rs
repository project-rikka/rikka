use rikka_physics_math::{Plane, Vec3};

pub(crate) const MAX_CLIP_POINTS: usize = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Pair {
    pub(crate) owner1: u8,
    pub(crate) index1: u16,
    pub(crate) owner2: u8,
    pub(crate) index2: u16,
}

pub(crate) const TRIANGLE: u8 = 0;
pub(crate) const CONVEX: u8 = 1;

impl Pair {
    pub(crate) const SINGLE: Self = Self::new(TRIANGLE, 0, TRIANGLE, 0);

    #[inline]
    pub(crate) const fn new(owner1: u8, index1: u16, owner2: u8, index2: u16) -> Self {
        Self {
            owner1,
            index1,
            owner2,
            index2,
        }
    }

    #[inline]
    pub(crate) fn id(self) -> u32 {
        u32::from(self.owner1 & 1) << 31
            | (u32::from(self.index1) & 0x7fff) << 16
            | u32::from(self.owner2 & 1) << 15
            | (u32::from(self.index2) & 0x7fff)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ClipPoint {
    pub(crate) position: Vec3,
    pub(crate) separation: f32,
    pub(crate) pair: Pair,
}

pub(crate) struct Polygon {
    pub(crate) points: [ClipPoint; MAX_CLIP_POINTS],
    pub(crate) count: usize,
}

impl Default for Polygon {
    fn default() -> Self {
        Self {
            points: [ClipPoint::default(); MAX_CLIP_POINTS],
            count: 0,
        }
    }
}
#[derive(Default)]
pub(crate) struct ClipBuffers {
    first: Polygon,
    second: Polygon,
}

impl ClipBuffers {
    #[inline]
    pub(crate) fn pair(&mut self) -> (&mut Polygon, &mut Polygon) {
        (&mut self.first, &mut self.second)
    }
}

impl std::fmt::Debug for ClipBuffers {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClipBuffers")
            .finish_non_exhaustive()
    }
}

impl Polygon {
    #[inline]
    pub(crate) fn push(&mut self, point: ClipPoint) {
        if self.count < MAX_CLIP_POINTS {
            self.points[self.count] = point;
            self.count += 1;
        }
    }

    #[inline]
    pub(crate) fn points(&self) -> &[ClipPoint] {
        &self.points[..self.count]
    }

    pub(crate) fn clip(
        &self,
        output: &mut Polygon,
        clip_plane: Plane,
        edge: u16,
        owner: u8,
        reference: Plane,
    ) {
        output.count = 0;
        let Some(&last) = self.points().last() else {
            return;
        };
        let mut vertex1 = last;
        let mut distance1 = clip_plane.separation(vertex1.position);
        for &vertex2 in self.points() {
            let distance2 = clip_plane.separation(vertex2.position);
            if distance1 <= 0.0 && distance2 <= 0.0 {
                output.push(vertex2);
            } else if distance1 <= 0.0 && distance2 > 0.0 {
                let fraction = distance1 / (distance1 - distance2);
                let position = vertex1.position.lerp(vertex2.position, fraction);
                let mut pair = vertex2.pair;
                pair.owner2 = owner;
                pair.index2 = edge;
                output.push(ClipPoint {
                    position,
                    separation: reference.separation(position),
                    pair,
                });
            } else if distance2 <= 0.0 && distance1 > 0.0 {
                let fraction = distance1 / (distance1 - distance2);
                let position = vertex1.position.lerp(vertex2.position, fraction);
                let mut pair = vertex1.pair;
                pair.owner1 = owner;
                pair.index1 = edge;
                output.push(ClipPoint {
                    position,
                    separation: reference.separation(position),
                    pair,
                });
                output.push(vertex2);
            }
            vertex1 = vertex2;
            distance1 = distance2;
        }
    }
}

#[inline]
pub(crate) fn plane_from_points(a: Vec3, b: Vec3, c: Vec3) -> Plane {
    let normal = (b - a).cross(c - a).normalize_or_zero();
    Plane::from_normal_point(normal, a)
}

#[inline]
pub(crate) fn line_closest(p1: Vec3, d1: Vec3, p2: Vec3, d2: Vec3) -> (Vec3, f32, Vec3, f32) {
    let r = p1 - p2;
    let a = d1.dot(d1);
    let b = d1.dot(d2);
    let c = d1.dot(r);
    let e = d2.dot(d2);
    let f = d2.dot(r);
    let denominator = a * e - b * b;
    let (s, t) =
        if denominator.abs() <= f32::EPSILON * a * e || a <= f32::EPSILON || e <= f32::EPSILON {
            (0.0, if e > f32::EPSILON { f / e } else { 0.0 })
        } else {
            let s = (b * f - c * e) / denominator;
            (s, (b * s + f) / e)
        };
    (p1 + s * d1, s, p2 + t * d2, t)
}
