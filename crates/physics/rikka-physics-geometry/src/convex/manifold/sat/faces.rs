use rikka_physics_math::{F32x4, Float, MaskW, Mat3, Vec3, Vector};

use super::{AxisKind, LANES, SeparatingAxis, row, valid_lanes};
use crate::convex::extrema::larger;
use crate::convex::hull::Polyhedron;

#[derive(Clone, Copy)]
pub(super) struct FaceSide<'p, 'a> {
    pub(super) reference: &'p Polyhedron<'a>,
    pub(super) other: &'p Polyhedron<'a>,
    pub(super) rotation: Mat3,
    pub(super) translation: Vec3,
    pub(super) kind: AxisKind,
}

impl FaceSide<'_, '_> {
    #[inline]
    pub(super) fn axis(&self, face: usize) -> SeparatingAxis {
        let plane = self.reference.planes[face];
        let direction = -(self.rotation * plane.normal);
        let separation =
            plane.normal.dot(self.translation) - plane.offset - self.other.max_dot(direction);
        match self.kind {
            AxisKind::FaceA => SeparatingAxis {
                normal: plane.normal,
                separation,
                index_a: face,
                index_b: 0,
                kind: AxisKind::FaceA,
            },
            _ => SeparatingAxis {
                normal: direction,
                separation,
                index_a: 0,
                index_b: face,
                kind: AxisKind::FaceB,
            },
        }
    }

    pub(super) fn complete(&self, axis: &mut SeparatingAxis) {
        match self.kind {
            AxisKind::FaceA => {
                let direction = -(self.rotation * axis.normal);
                axis.index_b = self.other.support_vertex(direction);
            }
            _ => axis.index_a = self.other.support_vertex(axis.normal),
        }
    }

    pub(super) fn search(
        &self,
        toward: Vec3,
        slack: f32,
        floor: f32,
        best: &mut SeparatingAxis,
        skip: usize,
        separated_above: f32,
    ) -> bool {
        let [nx, ny, nz] = self.reference.soa_face_normals();
        let distances = self.reference.face_distances;
        let count = self.reference.face_count();
        let dx = F32x4::splat(toward.x);
        let dy = F32x4::splat(toward.y);
        let dz = F32x4::splat(toward.z);
        let inner = F32x4::splat(self.other.inner_radius);
        let slack = F32x4::splat(slack);
        let reach = Reach::new(self.rotation, self.other.box_axes);
        for group in 0..count.div_ceil(LANES) {
            let normal = [row(nx, group), row(ny, group), row(nz, group)];
            let bound = match &reach {
                Some(reach) => reach.extent(normal).max(inner),
                None => inner,
            };
            let margins = normal[0] * dx + normal[1] * dy + normal[2] * dz
                - row(distances, group)
                - (bound - slack);
            let mut bits = margins
                .simd_ge(F32x4::splat(larger(floor, best.separation)))
                .bits()
                & valid_lanes(count - group * LANES);
            if bits == 0 {
                continue;
            }
            let margins = margins.to_array::<4>();
            while bits != 0 {
                let lane = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let face = group * LANES + lane;
                if face == skip || margins[lane] < larger(floor, best.separation) {
                    continue;
                }
                let axis = self.axis(face);
                if axis.separation > best.separation {
                    *best = axis;
                    if axis.separation > separated_above {
                        return true;
                    }
                }
            }
        }
        false
    }
}

struct Reach {
    axes: [[F32x4; 3]; 3],
}

impl Reach {
    #[inline]
    fn new(rotation: Mat3, axes: [Vec3; 3]) -> Option<Self> {
        if axes[0] == Vec3::ZERO {
            return None;
        }
        let local = axes.map(|axis| rotation.transpose() * axis);
        Some(Self {
            axes: local.map(|axis| {
                [
                    F32x4::splat(axis.x),
                    F32x4::splat(axis.y),
                    F32x4::splat(axis.z),
                ]
            }),
        })
    }

    #[inline(always)]
    fn extent(&self, normal: [F32x4; 3]) -> F32x4 {
        let reach = |axis: &[F32x4; 3]| {
            (normal[0] * axis[0] + normal[1] * axis[1] + normal[2] * axis[2]).abs()
        };
        reach(&self.axes[0]) + reach(&self.axes[1]) + reach(&self.axes[2])
    }
}
