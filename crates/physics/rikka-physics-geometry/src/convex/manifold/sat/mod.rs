mod edges;
mod faces;

use rikka_physics_math::{F32x4, Mat3, Transform, Vec3, Vector};

use crate::convex::extrema::larger;
use crate::convex::hull::Polyhedron;
use edges::EdgeQuery;
use faces::FaceSide;

const LANES: usize = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AxisKind {
    #[default]
    None,
    FaceA,
    FaceB,
    EdgePair,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SeparatingAxis {
    pub(crate) normal: Vec3,
    pub(crate) separation: f32,
    pub(crate) index_a: usize,
    pub(crate) index_b: usize,
    pub(crate) kind: AxisKind,
}

impl SeparatingAxis {
    const fn empty(kind: AxisKind) -> Self {
        Self {
            normal: Vec3::ZERO,
            separation: f32::NEG_INFINITY,
            index_a: usize::MAX,
            index_b: usize::MAX,
            kind,
        }
    }

    #[inline]
    pub(crate) fn is_valid(&self) -> bool {
        self.index_a != usize::MAX
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AxisQuery {
    pub(crate) face_a: SeparatingAxis,
    pub(crate) face_b: SeparatingAxis,
    pub(crate) edge: SeparatingAxis,
    pub(crate) separated: AxisKind,
}

#[inline(always)]
fn valid_lanes(remaining: usize) -> u32 {
    if remaining >= LANES {
        (1 << LANES) - 1
    } else {
        (1 << remaining) - 1
    }
}

#[inline(always)]
fn row(values: &[f32], group: usize) -> F32x4 {
    let (chunks, _) = values.as_chunks::<LANES>();
    F32x4::from_array(chunks[group])
}

pub(crate) fn separating_axis(
    a: &Polyhedron<'_>,
    b: &Polyhedron<'_>,
    transform: Transform,
    speculative: f32,
    linear_slop: f32,
    early_return: bool,
) -> AxisQuery {
    let rotation = Mat3::from_quat(transform.q);
    let inverse = rotation.transpose();
    let mut result = AxisQuery {
        face_a: SeparatingAxis::empty(AxisKind::FaceA),
        face_b: SeparatingAxis::empty(AxisKind::FaceB),
        edge: SeparatingAxis::empty(AxisKind::EdgePair),
        separated: AxisKind::None,
    };
    let separated_above = if early_return {
        speculative
    } else {
        f32::INFINITY
    };

    let side_a = FaceSide {
        reference: a,
        other: b,
        rotation: inverse,
        translation: transform.p,
        kind: AxisKind::FaceA,
    };
    let side_b = FaceSide {
        reference: b,
        other: a,
        rotation,
        translation: -(inverse * transform.p),
        kind: AxisKind::FaceB,
    };

    let delta_center = rotation * b.center + transform.p - a.center;
    let delta_in_b = -(inverse * delta_center);
    let center_distance = delta_center.length();
    let radius = a.inner_radius + b.inner_radius;
    let slack = linear_slop + 0.001 * (center_distance + radius);

    let mut floor = f32::NEG_INFINITY;
    let (mut seed_a, mut seed_b) = (usize::MAX, usize::MAX);
    if early_return {
        seed_a = a.aligned_face(delta_center);
        result.face_a = side_a.axis(seed_a);
        if result.face_a.separation > speculative {
            result.separated = AxisKind::FaceA;
            return result;
        }
        seed_b = b.aligned_face(delta_in_b);
        result.face_b = side_b.axis(seed_b);
        if result.face_b.separation > speculative {
            result.separated = AxisKind::FaceB;
            return result;
        }
        floor = larger(result.face_a.separation, result.face_b.separation);
    }

    if side_a.search(
        delta_center,
        slack,
        floor,
        &mut result.face_a,
        seed_a,
        separated_above,
    ) {
        result.separated = AxisKind::FaceA;
        return result;
    }
    if early_return {
        floor = larger(floor, result.face_a.separation);
    }
    if side_b.search(
        delta_in_b,
        slack,
        floor,
        &mut result.face_b,
        seed_b,
        separated_above,
    ) {
        result.separated = AxisKind::FaceB;
        return result;
    }

    let floor = if early_return {
        result.face_a.separation.max(result.face_b.separation) - slack
    } else {
        f32::NEG_INFINITY
    };
    let edges = EdgeQuery {
        rotation,
        translation: transform.p,
        toward_b: delta_center,
        toward_a: delta_in_b,
        center_distance,
        floor,
        separated_above,
    };
    if edges.search(a, b, &mut result.edge) {
        result.separated = AxisKind::EdgePair;
    } else if result.face_a.separation > result.face_b.separation {
        side_a.complete(&mut result.face_a);
    } else if result.face_b.is_valid() {
        side_b.complete(&mut result.face_b);
    }
    result
}
