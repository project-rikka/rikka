use rikka_physics_geometry::{PairCache, Shape, TriangulatedContact};
use rikka_physics_math::{Quat, Transform};

use crate::{ContactManifold, ContactPoint};

pub const TOUCHING: u32 = 1 << 0;
pub const STARTED_TOUCHING: u32 = 1 << 1;
pub const STOPPED_TOUCHING: u32 = 1 << 2;
pub const DISJOINT: u32 = 1 << 3;
pub(crate) const RECYCLE_VALID: u32 = 1 << 4;
pub const ENABLE_RECYCLE: u32 = 1 << 5;
pub const STATE_CHANGES: u32 = STARTED_TOUCHING | STOPPED_TOUCHING | DISJOINT;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PairKind {
    SphereSphere,
    SphereCapsule,
    CapsuleCapsule,
    SphereCuboid,
    CuboidCuboid,
    General,
    Triangulated,
}

impl PairKind {
    pub const COUNT: usize = 7;

    pub const ALL: [PairKind; Self::COUNT] = [
        PairKind::SphereSphere,
        PairKind::SphereCapsule,
        PairKind::CapsuleCapsule,
        PairKind::SphereCuboid,
        PairKind::CuboidCuboid,
        PairKind::General,
        PairKind::Triangulated,
    ];

    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn classify(a: &Shape, b: &Shape) -> (PairKind, bool) {
        if a.as_convex().is_none() && !matches!(a, Shape::Plane(_))
            || b.as_convex().is_none() && !matches!(b, Shape::Plane(_))
        {
            return (PairKind::Triangulated, false);
        }
        let rank = |shape: &Shape| match shape {
            Shape::Sphere(_) => Some(0),
            Shape::Capsule(capsule) if capsule.radius1 == capsule.radius2 => Some(1),
            Shape::Cuboid(_) => Some(2),
            _ => None,
        };
        let (Some(ra), Some(rb)) = (rank(a), rank(b)) else {
            return (PairKind::General, false);
        };
        let swap = ra > rb;
        let kind = match (ra.min(rb), ra.max(rb)) {
            (0, 0) => PairKind::SphereSphere,
            (0, 1) => PairKind::SphereCapsule,
            (1, 1) => PairKind::CapsuleCapsule,
            (0, 2) => PairKind::SphereCuboid,
            (2, 2) => PairKind::CuboidCuboid,
            _ => return (PairKind::General, false),
        };
        (kind, swap)
    }
}

#[repr(C, align(64))]
#[derive(Clone, Copy, Debug)]
pub struct ContactHeader {
    pub collider_a: u32,
    pub collider_b: u32,
    pub frame_a: u32,
    pub frame_b: u32,
    pub flags: u32,
    pub kind: PairKind,
    pub points: u8,
    pub extra: ExtraManifolds,
    pub cached_rotation_a: Quat,
    pub cached_rotation_b: Quat,
    pub cached_relative: Transform,
}

#[repr(C, align(64))]
#[derive(Clone, Debug, Default)]
pub struct ContactGeometry {
    pub manifold: ContactManifold,
    pub cache: PairCache,
    pub triangulated: Option<Box<TriangulatedContact>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExtraManifolds {
    pub worker: u32,
    pub start: u32,
    pub count: u32,
    pub stamp: u32,
}

impl ContactHeader {
    pub fn new(
        collider_a: u32,
        collider_b: u32,
        frame_a: u32,
        frame_b: u32,
        kind: PairKind,
        recycle: bool,
    ) -> Self {
        Self {
            collider_a,
            collider_b,
            frame_a,
            frame_b,
            flags: if recycle { ENABLE_RECYCLE } else { 0 },
            kind,
            points: 0,
            extra: ExtraManifolds::default(),
            cached_rotation_a: Quat::IDENTITY,
            cached_rotation_b: Quat::IDENTITY,
            cached_relative: Transform::IDENTITY,
        }
    }

    #[inline]
    pub fn is_touching(&self) -> bool {
        self.flags & TOUCHING != 0
    }
}

impl ContactGeometry {
    pub fn new(kind: PairKind) -> Self {
        Self {
            triangulated: (kind == PairKind::Triangulated).then(Box::default),
            ..Self::default()
        }
    }

    pub(crate) fn recycle_bytes(points: u8) -> usize {
        std::mem::offset_of!(ContactManifold, points) + points as usize * size_of::<ContactPoint>()
    }
}
