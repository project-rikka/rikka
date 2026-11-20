use super::clip::{ClipVertex, FeaturePair, Owner, clip_polygon};
use super::polyhedra::{Contact, SatCache};
use super::reduce::reduce;
use super::round::point;
use super::sat::AxisKind;
use crate::Manifold;
use crate::convex::cuboid::{Cuboid, signs};
use crate::convex::distance::line_distance;
use crate::convex::extrema::smaller;
use crate::convex::hull::{
    BOX_EDGE_AXES, BOX_FACE_LOOPS, box_edge, box_edge_corners, corner_point,
};
use rikka_physics_math::{F32x4, Float, MaskW, Mat3, Plane, Transform, Vec3, Vector};

const EDGE_TOLERANCE: f32 = 1.0e-5;
const CLIP_POINTS: usize = 8;

#[inline(always)]
fn face_index(axis: usize, sign: f32) -> usize {
    2 * axis + usize::from(sign > 0.0)
}

#[inline(always)]
fn face_sign(face: usize) -> f32 {
    if face % 2 == 1 { 1.0 } else { -1.0 }
}

#[derive(Clone, Copy, Debug)]
struct Axis {
    separation: f32,
    normal: Vec3,
    index_a: usize,
    index_b: usize,
}

impl Axis {
    const NONE: Self = Self {
        separation: f32::NEG_INFINITY,
        normal: Vec3::ZERO,
        index_a: usize::MAX,
        index_b: usize::MAX,
    };
}

#[derive(Clone, Copy, Debug)]
struct EdgeCandidate {
    index_a: usize,
    index_b: usize,
    separation: f32,
    offset: f32,
    length: f32,
    axis: Vec3,
}

impl EdgeCandidate {
    #[inline(always)]
    fn into_axis(self) -> Axis {
        let sign = if self.offset < 0.0 { -1.0 } else { 1.0 };
        Axis {
            separation: self.separation,
            normal: (sign / self.length) * self.axis,
            index_a: self.index_a,
            index_b: self.index_b,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Axes {
    face_a: Axis,
    face_b: Axis,
    edge: Axis,
    separated: AxisKind,
}

#[derive(Clone, Copy, Debug)]
struct Placed {
    half_extents: Vec3,
    rotation: Mat3,
    center: Vec3,
}

impl Placed {
    #[inline(always)]
    fn corner(&self, corner: usize) -> Vec3 {
        self.rotation * corner_point(corner, self.half_extents) + self.center
    }
}

#[derive(Clone, Copy, Debug)]
struct BoxPair {
    half_a: Vec3,
    half_b: Vec3,
    rotation: Mat3,
    absolute: Mat3,
    center: Vec3,
}

impl BoxPair {
    fn new(half_a: Vec3, half_b: Vec3, transform: Transform) -> Self {
        let rotation = Mat3::from_quat(transform.q);
        Self {
            half_a,
            half_b,
            rotation,
            absolute: Mat3::from_cols(
                rotation.x_axis.abs(),
                rotation.y_axis.abs(),
                rotation.z_axis.abs(),
            ),
            center: transform.p,
        }
    }

    #[inline(always)]
    fn face_a_separation(&self, axis: usize, sign: f32) -> f32 {
        sign * self.center[axis] - self.half_a[axis] - self.absolute.row(axis).dot(self.half_b)
    }

    #[inline(always)]
    fn face_b_separation(&self, axis: usize, sign: f32) -> f32 {
        let column = self.rotation.col(axis);
        sign * column.dot(self.center)
            - self.half_b[axis]
            - self.half_a.dot(self.absolute.col(axis))
    }

    #[inline(always)]
    fn edge_axis(&self, i: usize, j: usize) -> Option<(Vec3, f32)> {
        let col = self.rotation.col(j);
        let axis = match i {
            0 => Vec3::new(0.0, -col.z, col.y),
            1 => Vec3::new(col.z, 0.0, -col.x),
            _ => Vec3::new(-col.y, col.x, 0.0),
        };
        let length_squared = axis.length_squared();
        if length_squared < EDGE_TOLERANCE * EDGE_TOLERANCE {
            return None;
        }
        let reach_a = self.half_a.dot(axis.abs());
        let r = self.absolute.row(i);
        let hb = self.half_b;
        let reach_b = match j {
            0 => hb.y * r.z + hb.z * r.y,
            1 => hb.x * r.z + hb.z * r.x,
            _ => hb.x * r.y + hb.y * r.x,
        };
        let offset = axis.dot(self.center);
        let length = length_squared.sqrt();
        let separation = (offset.abs() - reach_a - reach_b) / length;
        let sign = if offset < 0.0 { -1.0 } else { 1.0 };
        Some(((sign / length) * axis, separation))
    }

    fn axes(&self, threshold: f32) -> Axes {
        let mut axes = Axes {
            face_a: Axis::NONE,
            face_b: Axis::NONE,
            edge: Axis::NONE,
            separated: AxisKind::None,
        };
        for i in 0..3 {
            let sign = if self.center[i] < 0.0 { -1.0 } else { 1.0 };
            let separation = self.face_a_separation(i, sign);
            if separation > axes.face_a.separation {
                axes.face_a = Axis {
                    separation,
                    normal: sign * Vec3::AXES[i],
                    index_a: face_index(i, sign),
                    index_b: 0,
                };
                if separation > threshold {
                    axes.separated = AxisKind::FaceA;
                    return axes;
                }
            }
        }
        for j in 0..3 {
            let column = self.rotation.col(j);
            let projection = column.dot(self.center);
            let sign = if projection < 0.0 { -1.0 } else { 1.0 };
            let separation =
                projection.abs() - self.half_b[j] - self.half_a.dot(self.absolute.col(j));
            if separation > axes.face_b.separation {
                axes.face_b = Axis {
                    separation,
                    normal: sign * column,
                    index_a: 0,
                    index_b: face_index(j, -sign),
                };
                if separation > threshold {
                    axes.separated = AxisKind::FaceB;
                    return axes;
                }
            }
        }
        let rows = [
            self.rotation.row(0),
            self.rotation.row(1),
            self.rotation.row(2),
        ]
        .map(|row| F32x4::from_array([row.x, row.y, row.z, 0.0]));
        let zero = F32x4::default();
        let tol_sq = F32x4::splat(EDGE_TOLERANCE * EDGE_TOLERANCE);
        let ha_x = F32x4::splat(self.half_a.x);
        let ha_y = F32x4::splat(self.half_a.y);
        let ha_z = F32x4::splat(self.half_a.z);
        let hb = self.half_b;
        let hb_0 = F32x4::from_array([hb.y, hb.x, hb.x, 0.0]);
        let hb_1 = F32x4::from_array([hb.z, hb.z, hb.y, 0.0]);
        let cx = F32x4::splat(self.center.x);
        let cy = F32x4::splat(self.center.y);
        let cz = F32x4::splat(self.center.z);
        let mut best_signed_sq = f32::NEG_INFINITY;
        let mut best_edge: Option<EdgeCandidate> = None;
        for i in 0..3 {
            let [x, y, z] = match i {
                0 => [zero, -rows[2], rows[1]],
                1 => [rows[2], zero, -rows[0]],
                _ => [-rows[1], rows[0], zero],
            };
            let length_squared = x * x + y * y + z * z;
            let valid = length_squared.simd_ge(tol_sq);
            if valid.bits() & 0b111 == 0 {
                continue;
            }
            let r = self.absolute.row(i);
            let reach_b = hb_0 * F32x4::from_array([r.z, r.z, r.y, 0.0])
                + hb_1 * F32x4::from_array([r.y, r.x, r.x, 0.0]);
            let reach_a = ha_x * x.abs() + ha_y * y.abs() + ha_z * z.abs();
            let offset = x * cx + y * cy + z * cz;
            let signed_dist = offset.abs() - reach_a - reach_b;
            let candidates = valid
                & (signed_dist * signed_dist.abs())
                    .simd_ge(F32x4::splat(best_signed_sq) * length_squared);
            let mut mask = candidates.bits() & 0b111;
            if mask == 0 {
                continue;
            }
            let length = length_squared.sqrt();
            let separation = (signed_dist / length).to_array::<4>();
            let offset = offset.to_array::<4>();
            let length = length.to_array::<4>();
            let ax = x.to_array::<4>();
            let ay = y.to_array::<4>();
            let az = z.to_array::<4>();
            while mask != 0 {
                let j = mask.trailing_zeros() as usize;
                mask &= mask - 1;
                let sep = separation[j];
                if sep > axes.edge.separation {
                    let candidate = EdgeCandidate {
                        index_a: i,
                        index_b: j,
                        separation: sep,
                        offset: offset[j],
                        length: length[j],
                        axis: Vec3::new(ax[j], ay[j], az[j]),
                    };
                    axes.edge.separation = sep;
                    if sep > threshold {
                        axes.edge = candidate.into_axis();
                        axes.separated = AxisKind::EdgePair;
                        return axes;
                    }
                    let signed_sq = sep * sep.abs();
                    best_signed_sq = signed_sq - 4.0 * f32::EPSILON * signed_sq.abs();
                    best_edge = Some(candidate);
                }
            }
        }
        if let Some(candidate) = best_edge {
            axes.edge = candidate.into_axis();
        }
        axes
    }

    fn supporting_edges(&self, i: usize, j: usize, normal: Vec3) -> (usize, usize) {
        let along_b = -(self.rotation.transpose() * normal);
        (box_edge(i, signs(normal)), box_edge(j, signs(along_b)))
    }

    fn face_contact(&self, face: usize, contact: Contact) -> Option<(Manifold, f32)> {
        let incident = Placed {
            half_extents: self.half_b,
            rotation: self.rotation,
            center: self.center,
        };
        clip_face(self.half_a, face, &incident, contact)
    }

    fn face_b_contact(&self, face: usize, contact: Contact) -> Option<(Manifold, f32)> {
        let inverse = self.rotation.transpose();
        let incident = Placed {
            half_extents: self.half_a,
            rotation: inverse,
            center: -(inverse * self.center),
        };
        let (mut manifold, separation) =
            clip_face(self.half_b, face, &incident, contact.swapped())?;
        manifold.normal = -(self.rotation * manifold.normal);
        for index in 0..manifold.count as usize {
            let entry = &mut manifold.points[index];
            entry.point = self.rotation * entry.point + self.center;
            entry.feature = FeaturePair::flip_id(entry.feature);
        }
        Some((manifold, separation))
    }

    fn edge_contact(&self, normal: Vec3, edge_a: usize, edge_b: usize) -> Option<(Manifold, f32)> {
        let placed_b = Placed {
            half_extents: self.half_b,
            rotation: self.rotation,
            center: self.center,
        };
        let loop_a = box_edge_corners(edge_a);
        let loop_b = box_edge_corners(edge_b);
        let pa = corner_point(loop_a[0], self.half_a);
        let qa = corner_point(loop_a[1], self.half_a);
        let pb = placed_b.corner(loop_b[0]);
        let qb = placed_b.corner(loop_b[1]);
        let result = line_distance(pa, qa - pa, pb, qb - pb);
        if !result.is_within_segments() {
            return None;
        }
        let separation = normal.dot(result.point2 - result.point1);
        let mut manifold = Manifold {
            normal,
            ..Manifold::default()
        };
        manifold.push(point(
            0.5 * (result.point1 + result.point2),
            separation,
            FeaturePair::new(Owner::A, edge_a as u16, Owner::B, edge_b as u16),
        ));
        Some((manifold, separation))
    }

    fn cached(&self, contact: Contact, cache: &SatCache) -> Option<Manifold> {
        let threshold = contact.threshold();
        let rebuilt = match cache.kind {
            AxisKind::None => return None,
            AxisKind::FaceA => {
                let face = cache.index_a as usize;
                if self.face_a_separation(face / 2, face_sign(face)) >= threshold {
                    return Some(Manifold::default());
                }
                self.face_contact(face, contact)
            }
            AxisKind::FaceB => {
                let face = cache.index_b as usize;
                if self.face_b_separation(face / 2, -face_sign(face)) >= threshold {
                    return Some(Manifold::default());
                }
                self.face_b_contact(face, contact)
            }
            AxisKind::EdgePair => {
                let (edge_a, edge_b) = (cache.index_a as usize, cache.index_b as usize);
                let (i, j) = (BOX_EDGE_AXES[edge_a / 2], BOX_EDGE_AXES[edge_b / 2]);
                let (normal, separation) = self.edge_axis(i, j)?;
                if separation > threshold {
                    return Some(Manifold::default());
                }
                if self.supporting_edges(i, j, normal) != (edge_a, edge_b) {
                    return None;
                }
                self.edge_contact(normal, edge_a, edge_b)
            }
        };
        let (manifold, separation) = rebuilt?;
        ((cache.separation - separation).abs() < contact.linear_slop).then_some(manifold)
    }

    fn collide(&self, contact: Contact, cache: &mut SatCache) -> Manifold {
        if let Some(manifold) = self.cached(contact, cache) {
            return manifold;
        }
        cache.reset();

        let axes = self.axes(contact.threshold());
        if axes.separated != AxisKind::None {
            let axis = match axes.separated {
                AxisKind::FaceA => axes.face_a,
                AxisKind::FaceB => axes.face_b,
                _ => {
                    let (edge_a, edge_b) = self.supporting_edges(
                        axes.edge.index_a,
                        axes.edge.index_b,
                        axes.edge.normal,
                    );
                    Axis {
                        index_a: edge_a,
                        index_b: edge_b,
                        ..axes.edge
                    }
                }
            };
            cache.store(axes.separated, axis.separation, axis.index_a, axis.index_b);
            return Manifold::default();
        }

        let mut manifold = Manifold::default();
        let built = if axes.face_a.separation > axes.face_b.separation {
            self.face_contact(axes.face_a.index_a, contact)
                .map(|built| (built, AxisKind::FaceA, axes.face_a))
        } else {
            self.face_b_contact(axes.face_b.index_b, contact)
                .map(|built| (built, AxisKind::FaceB, axes.face_b))
        };
        if let Some(((built, separation), kind, face)) = built {
            manifold = built;
            cache.store(kind, separation, face.index_a, face.index_b);
        }

        let edge = axes.edge;
        if edge.index_a == usize::MAX {
            return manifold;
        }
        if manifold.is_empty() || edge.separation > cache.separation + contact.linear_slop {
            let (edge_a, edge_b) = self.supporting_edges(edge.index_a, edge.index_b, edge.normal);
            if let Some((edge_manifold, separation)) =
                self.edge_contact(edge.normal, edge_a, edge_b)
            {
                cache.store(AxisKind::EdgePair, separation, edge_a, edge_b);
                return edge_manifold;
            }
        }
        manifold
    }
}

fn clip_face(
    half: Vec3,
    face: usize,
    incident: &Placed,
    contact: Contact,
) -> Option<(Manifold, f32)> {
    let axis = face / 2;
    let sign = face_sign(face);
    let normal = sign * Vec3::AXES[axis];
    let reference = Plane::new(normal, half[axis]);
    let local = incident.rotation.row(axis) * sign;
    let facing = local.abs().max_position();
    let incident_face = face_index(facing, -local[facing]);

    let mut first = [ClipVertex::default(); CLIP_POINTS];
    let mut second = [ClipVertex::default(); CLIP_POINTS];
    let loop_b = &BOX_FACE_LOOPS[incident_face];
    for (slot, vertex) in first.iter_mut().take(4).enumerate() {
        let position = incident.corner(loop_b.corners[slot]);
        let edge = loop_b.edges[slot];
        let next = loop_b.edges[(slot + 1) % 4];
        *vertex = ClipVertex {
            position: position.extend(0.0),
            separation: sign * position[axis] - half[axis],
            pair: FeaturePair::new(Owner::B, edge, Owner::B, next),
        };
    }

    let mut count = 4;
    let mut input = &mut first;
    let mut output = &mut second;
    for side in &BOX_FACE_LOOPS[face].sides {
        let plane = Plane::new(side.sign * Vec3::AXES[side.axis], half[side.axis]);
        if let Some(clipped) = clip_polygon(output, &input[..count], plane, side.edge, reference) {
            count = clipped;
            std::mem::swap(&mut input, &mut output);
        }
        if count < 3 {
            return None;
        }
    }

    let mut min_separation = f32::MAX;
    for vertex in &mut input[..count] {
        let offset = 0.5 * (contact.radius_a - contact.radius_b - vertex.separation);
        vertex.position += (offset * normal).extend(0.0);
        vertex.separation -= contact.radius();
        min_separation = smaller(min_separation, vertex.separation);
    }
    if min_separation >= contact.speculative {
        return None;
    }
    let mut manifold = Manifold {
        normal,
        ..Manifold::default()
    };
    reduce(&mut manifold, &mut input[..count], contact.speculative);
    Some((manifold, min_separation))
}

pub(crate) fn collide_cuboids(
    a: &Cuboid,
    b: &Cuboid,
    transform: Transform,
    contact: Contact,
    cache: &mut SatCache,
) -> Manifold {
    let placed = a.transform.inverse_mul(&transform.mul(&b.transform));
    let pair = BoxPair::new(a.half_extents, b.half_extents, placed);
    let mut manifold = pair.collide(contact, cache);
    if a.transform != Transform::IDENTITY {
        manifold.normal = a.transform.rotate(manifold.normal);
        for index in 0..manifold.count as usize {
            let entry = &mut manifold.points[index];
            entry.point = a.transform.apply(entry.point);
        }
    }
    manifold
}
