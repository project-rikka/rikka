use rikka_physics_math::{Plane, Transform, Vec3};

use super::clip::{ClipVertex, clip_segment};
use super::hull_round::{
    CapsuleFeatures, SHALLOW, capsule_segment, closest_contact, deep_capsule, two_point,
};
use crate::convex::capsule::Capsule;
use crate::convex::cuboid::Cuboid;
use crate::convex::distance::{DistanceInput, SimplexCache, shape_distance};
use crate::convex::hull::{
    BOX_EDGE_AXES, BOX_FACE_LOOPS, BOX_PAIR_FACES, box_edge_corners, corner_point,
};
use crate::convex::support::{Support, SupportPoint};
use crate::{Manifold, Tolerance};

const FLAT: f32 = 0.998;

#[derive(Clone, Copy, Debug)]
struct AlignedBox(Vec3);

#[derive(Clone, Copy, Debug)]
struct Segment([Vec3; 2]);

impl Support for Segment {
    #[inline]
    fn support(&self, direction: Vec3) -> SupportPoint {
        let [start, end] = self.0;
        let far = direction.dot(end - start) > 0.0;
        SupportPoint::new(if far { end } else { start }, u32::from(far))
    }

    #[inline]
    fn radius(&self) -> f32 {
        0.0
    }

    #[inline]
    fn anchor(&self) -> SupportPoint {
        SupportPoint::new(self.0[0], 0)
    }

    #[inline]
    fn vertex(&self, id: u32) -> Option<Vec3> {
        self.0.get(id as usize).copied()
    }
}

impl Support for AlignedBox {
    #[inline]
    fn support(&self, direction: Vec3) -> SupportPoint {
        let [low_x, low_y, low_z] = [direction.x < 0.0, direction.y < 0.0, direction.z < 0.0];
        let pick = |low: bool, extent: f32| if low { -extent } else { extent };
        let point = Vec3::new(
            pick(low_x, self.0.x),
            pick(low_y, self.0.y),
            pick(low_z, self.0.z),
        );
        let [x, y, z] = [low_x, low_y, low_z].map(u32::from);
        SupportPoint::new(point, 2 * y + (x ^ y) + 4 * z)
    }

    #[inline]
    fn radius(&self) -> f32 {
        0.0
    }

    #[inline]
    fn vertex(&self, id: u32) -> Option<Vec3> {
        (id < 8).then(|| corner_point(id as usize, self.0))
    }
}

fn face_contact(
    half: Vec3,
    points: [Vec3; 2],
    normal: Vec3,
    radius: f32,
    speculative: f32,
) -> Option<Manifold> {
    let axis = normal.abs().max_position();
    if normal[axis].abs() <= FLAT {
        return None;
    }
    let sign = if normal[axis] < 0.0 { -1.0 } else { 1.0 };
    let face = 2 * axis + usize::from(sign > 0.0);
    let mut segment = capsule_segment(points);
    if AlignedBox(half).clip_to_face(&mut segment, face) < 2 {
        return None;
    }
    let distances = segment.map(|vertex| sign * vertex.point()[axis] - half[axis]);
    let reach = radius + speculative;
    (distances[0] <= reach || distances[1] <= reach)
        .then(|| two_point(sign * Vec3::AXES[axis], &segment, distances, radius))
}

pub(crate) fn collide_cuboid_capsule(
    cuboid: &Cuboid,
    capsule: &Capsule,
    transform: Transform,
    speculative: f32,
    tolerance: Tolerance,
    cache: &mut SimplexCache,
) -> Manifold {
    let local = cuboid.transform.inverse_mul(&transform);
    let half = cuboid.half_extents;
    let segment = [capsule.center1, capsule.center2];
    let points = segment.map(|point| local.apply(point));
    let radius = capsule.radius1;
    let reach = half + Vec3::splat(radius + speculative);
    let low = points[0].min(points[1]);
    let high = points[0].max(points[1]);
    if low.cmpgt(reach).any() || high.cmplt(-reach).any() {
        cache.reset();
        return Manifold::default();
    }
    let output = shape_distance(
        &DistanceInput::new(&AlignedBox(half), &Segment(segment), local),
        cache,
    );
    if output.distance > radius + speculative {
        cache.reset();
        return Manifold::default();
    }
    let mut manifold = if output.distance > SHALLOW {
        face_contact(half, points, output.normal, radius, speculative)
            .unwrap_or_else(|| closest_contact(&output, radius))
    } else {
        deep_capsule(&AlignedBox(half), points, radius, speculative, tolerance)
    };
    if cuboid.transform != Transform::IDENTITY {
        manifold.normal = cuboid.transform.rotate(manifold.normal);
        for index in 0..manifold.count as usize {
            let entry = &mut manifold.points[index];
            entry.point = cuboid.transform.apply(entry.point);
        }
    }
    manifold
}

impl CapsuleFeatures for AlignedBox {
    #[inline]
    fn planes(&self) -> impl Iterator<Item = Plane> {
        (0..6).map(|face| self.plane(face))
    }

    #[inline]
    fn plane(&self, face: usize) -> Plane {
        Plane::new(face_normal(face), self.0[face / 2])
    }

    #[inline]
    fn edge_pair_count(&self) -> usize {
        BOX_EDGE_AXES.len()
    }

    #[inline]
    fn edge_normals(&self, pair: usize) -> (Vec3, Vec3) {
        let [first, second] = BOX_PAIR_FACES[pair];
        (face_normal(first), face_normal(second))
    }

    #[inline]
    fn edge_points(&self, edge: usize) -> (Vec3, Vec3) {
        let [from, to] = box_edge_corners(edge);
        (corner_point(from, self.0), corner_point(to, self.0))
    }

    fn clip_to_face(&self, segment: &mut [ClipVertex; 2], face: usize) -> usize {
        for side in &BOX_FACE_LOOPS[face].sides {
            let plane = Plane::new(side.sign * Vec3::AXES[side.axis], self.0[side.axis]);
            if clip_segment(segment, plane) < 2 {
                return 0;
            }
        }
        2
    }
}

#[inline(always)]
fn face_normal(face: usize) -> Vec3 {
    if face % 2 == 1 {
        Vec3::AXES[face / 2]
    } else {
        -Vec3::AXES[face / 2]
    }
}
