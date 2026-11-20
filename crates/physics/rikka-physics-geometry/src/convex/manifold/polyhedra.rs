use rikka_physics_math::{Mat3, Transform, Vec3};

use super::clip::{ClipVertex, FeaturePair, MAX_CLIP_POINTS, Owner, clip_polygon};
use super::collide::PairCache;
use super::faces::FLAT;
use super::reduce::reduce;
use super::round::point;
use super::sat::{AxisKind, AxisQuery, SeparatingAxis};
use crate::Manifold;
use crate::convex::distance::{DistanceInput, SimplexCache, line_distance, shape_distance};
use crate::convex::extrema::smaller;
use crate::convex::hull::Polyhedron;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SatCache {
    pub(crate) separation: f32,
    pub(crate) kind: AxisKind,
    pub(crate) index_a: u16,
    pub(crate) index_b: u16,
}

impl SatCache {
    pub const EMPTY: Self = Self {
        separation: 0.0,
        kind: AxisKind::None,
        index_a: 0,
        index_b: 0,
    };

    #[inline]
    pub fn reset(&mut self) {
        *self = Self::EMPTY;
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.kind == AxisKind::None
    }

    pub(crate) fn store(
        &mut self,
        kind: AxisKind,
        separation: f32,
        index_a: usize,
        index_b: usize,
    ) {
        *self = Self {
            separation,
            kind,
            index_a: index_a as u16,
            index_b: index_b as u16,
        };
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Contact {
    pub(crate) speculative: f32,
    pub(crate) linear_slop: f32,
    pub(crate) radius_a: f32,
    pub(crate) radius_b: f32,
}

impl Contact {
    #[inline]
    pub(crate) fn radius(&self) -> f32 {
        self.radius_a + self.radius_b
    }

    #[inline]
    pub(crate) fn threshold(&self) -> f32 {
        self.speculative + self.radius()
    }

    #[inline]
    pub(crate) fn swapped(self) -> Self {
        Self {
            radius_a: self.radius_b,
            radius_b: self.radius_a,
            ..self
        }
    }
}

const SMALL_CLIP_POINTS: usize = 16;

#[derive(Clone, Copy, Debug)]
struct Overflow;

#[derive(Clone, Copy)]
struct Frame {
    rotation: Mat3,
    translation: Vec3,
}

impl Frame {
    #[inline]
    fn from_transform(transform: Transform) -> Self {
        Self {
            rotation: Mat3::from_quat(transform.q),
            translation: transform.p,
        }
    }

    #[inline]
    fn inverse(&self) -> Self {
        let rotation = self.rotation.transpose();
        Self {
            rotation,
            translation: -(rotation * self.translation),
        }
    }

    #[inline(always)]
    fn apply(&self, point: Vec3) -> Vec3 {
        self.rotation * point + self.translation
    }

    #[inline(always)]
    fn apply_inverse(&self, point: Vec3) -> Vec3 {
        self.rotate_inverse(point - self.translation)
    }

    #[inline(always)]
    fn rotate(&self, vector: Vec3) -> Vec3 {
        self.rotation * vector
    }

    #[inline(always)]
    fn rotate_inverse(&self, vector: Vec3) -> Vec3 {
        self.rotation.transpose() * vector
    }
}

fn clip_faces(
    a: &Polyhedron<'_>,
    b: &Polyhedron<'_>,
    frame: Frame,
    reference_face: usize,
    vertex_b: usize,
    contact: Contact,
) -> Option<(Manifold, f32)> {
    let reference = a.planes[reference_face];
    let incident = b.incident_face(frame.rotate_inverse(reference.normal), vertex_b);
    match clip_with::<SMALL_CLIP_POINTS>(a, b, frame, reference_face, incident, contact) {
        Ok(clipped) => clipped,
        Err(Overflow) => clip_large(a, b, frame, reference_face, incident, contact),
    }
}

#[inline(never)]
fn clip_large(
    a: &Polyhedron<'_>,
    b: &Polyhedron<'_>,
    frame: Frame,
    reference_face: usize,
    incident: usize,
    contact: Contact,
) -> Option<(Manifold, f32)> {
    clip_with::<MAX_CLIP_POINTS>(a, b, frame, reference_face, incident, contact).unwrap_or(None)
}

fn clip_with<const N: usize>(
    a: &Polyhedron<'_>,
    b: &Polyhedron<'_>,
    frame: Frame,
    reference_face: usize,
    incident: usize,
    contact: Contact,
) -> Result<Option<(Manifold, f32)>, Overflow> {
    let truncates = N >= MAX_CLIP_POINTS;
    let reference = a.planes[reference_face];
    let mut first = [ClipVertex::default(); N];
    let mut second = [ClipVertex::default(); N];
    let mut count = 0;
    for edge in b.face_edges(incident) {
        if count == N {
            if truncates {
                break;
            }
            return Err(Overflow);
        }
        let next = b.edges[edge].next;
        let position = frame.apply(b.points[b.edges[next as usize].origin as usize]);
        first[count] = ClipVertex {
            position: position.extend(0.0),
            separation: reference.separation(position),
            pair: FeaturePair::new(Owner::B, edge as u16, Owner::B, next),
        };
        count += 1;
    }

    let mut input = &mut first;
    let mut output = &mut second;
    for edge in a.face_edges(reference_face) {
        let (vertex1, vertex2) = (
            a.points[a.edges[edge].origin as usize],
            a.points[a.edges[a.edges[edge].next as usize].origin as usize],
        );
        let binormal = (vertex2 - vertex1).cross(reference.normal);
        let plane = rikka_physics_math::Plane::from_normal_point(binormal, vertex1);
        if let Some(clipped) = clip_polygon(output, &input[..count], plane, edge as u16, reference)
        {
            count = clipped;
            std::mem::swap(&mut input, &mut output);
        }
        if !truncates && count + 1 >= N {
            return Err(Overflow);
        }
        if count < 3 {
            return Ok(None);
        }
    }

    let mut min_separation = f32::MAX;
    for vertex in &mut input[..count] {
        let offset = 0.5 * (contact.radius_a - contact.radius_b - vertex.separation);
        vertex.position += (offset * reference.normal).extend(0.0);
        vertex.separation -= contact.radius();
        min_separation = smaller(min_separation, vertex.separation);
    }
    if min_separation >= contact.speculative {
        return Ok(None);
    }
    let mut manifold = Manifold {
        normal: reference.normal,
        ..Manifold::default()
    };
    reduce(&mut manifold, &mut input[..count], contact.speculative);
    Ok(Some((manifold, min_separation)))
}

fn face_b_contact(
    a: &Polyhedron<'_>,
    b: &Polyhedron<'_>,
    frame: Frame,
    vertex_a: usize,
    reference_face: usize,
    contact: Contact,
) -> Option<(Manifold, f32)> {
    let (mut manifold, separation) = clip_faces(
        b,
        a,
        frame.inverse(),
        reference_face,
        vertex_a,
        contact.swapped(),
    )?;
    manifold.normal = -(frame.rotation * manifold.normal);
    for index in 0..manifold.count as usize {
        let entry = &mut manifold.points[index];
        entry.point = frame.apply(entry.point);
        entry.feature = FeaturePair::flip_id(entry.feature);
    }
    Some((manifold, separation))
}

fn edge_contact(
    a: &Polyhedron<'_>,
    b: &Polyhedron<'_>,
    frame: Frame,
    normal: Vec3,
    edge_a: usize,
    edge_b: usize,
    contact: Contact,
) -> Option<(Manifold, f32)> {
    let (pa, qa) = a.edge_points(edge_a);
    let (pb, qb) = b.edge_points(edge_b);
    let pb = frame.apply(pb);
    let qb = frame.apply(qb);
    let result = line_distance(pa, qa - pa, pb, qb - pb);
    if !result.is_within_segments() {
        return None;
    }
    let separation = normal.dot(result.point2 - result.point1) - contact.radius();
    let mut manifold = Manifold {
        normal,
        ..Manifold::default()
    };
    manifold.push(point(
        0.5 * (result.point1 + contact.radius_a * normal + result.point2
            - contact.radius_b * normal),
        separation,
        FeaturePair::new(Owner::A, edge_a as u16, Owner::B, edge_b as u16),
    ));
    Some((manifold, separation))
}

fn cached(
    a: &Polyhedron<'_>,
    b: &Polyhedron<'_>,
    frame: Frame,
    contact: Contact,
    cache: &SatCache,
) -> Option<Manifold> {
    let index_a = cache.index_a as usize;
    let index_b = cache.index_b as usize;
    let rebuilt = match cache.kind {
        AxisKind::None => return None,
        AxisKind::FaceA => {
            let plane = a.planes[index_a];
            let vertex = b.support_vertex(-frame.rotate_inverse(plane.normal));
            if plane.separation(frame.apply(b.points[vertex])) >= contact.threshold() {
                return Some(Manifold::default());
            }
            clip_faces(a, b, frame, index_a, vertex, contact)
        }
        AxisKind::FaceB => {
            let plane = b.planes[index_b];
            let vertex = a.support_vertex(-frame.rotate(plane.normal));
            if plane.separation(frame.apply_inverse(a.points[vertex])) >= contact.threshold() {
                return Some(Manifold::default());
            }
            face_b_contact(a, b, frame, vertex, index_b, contact)
        }
        AxisKind::EdgePair => {
            let (pa, qa) = a.edge_points(index_a);
            let ea = qa - pa;
            let ua = a.planes[a.edges[index_a].face as usize].normal;
            let va = a.planes[a.edges[index_a + 1].face as usize].normal;
            let (pb, qb) = b.edge_points(index_b);
            let pb = frame.apply(pb);
            let qb = frame.apply(qb);
            let eb = qb - pb;
            let ub = frame.rotate(b.planes[b.edges[index_b].face as usize].normal);
            let vb = frame.rotate(b.planes[b.edges[index_b + 1].face as usize].normal);
            let cba = ub.dot(ea);
            let dba = vb.dot(ea);
            let adc = -ua.dot(eb);
            let bdc = -va.dot(eb);
            let tolerance = 0.005 * 0.005;
            if !(cba * dba < 0.0 && adc * bdc < 0.0 && cba * bdc > 0.0)
                || (cba * cba).max(dba * dba) < tolerance * ea.length_squared()
            {
                return None;
            }
            let t = cba / (cba - dba);
            let axis = ub.lerp(vb, t).normalize();
            if axis.dot(qa - qb) > contact.threshold() {
                return Some(Manifold::default());
            }
            edge_contact(a, b, frame, -axis, index_a, index_b, contact)
        }
    };
    let (manifold, separation) = rebuilt?;
    ((cache.separation - separation).abs() < contact.linear_slop).then_some(manifold)
}

fn best_face(query: &AxisQuery) -> SeparatingAxis {
    if query.face_a.separation > query.face_b.separation {
        query.face_a
    } else {
        query.face_b
    }
}

fn rounded_separated(
    a: &Polyhedron<'_>,
    b: &Polyhedron<'_>,
    transform: Transform,
    frame: Frame,
    contact: Contact,
    simplex: &mut SimplexCache,
    cache: &mut SatCache,
) -> Option<Manifold> {
    let output = shape_distance(&DistanceInput::new(a, b, transform), simplex);
    if output.distance > contact.threshold() {
        cache.reset();
        return Some(Manifold::default());
    }
    if output.distance <= contact.linear_slop {
        return None;
    }
    let normal = output.normal;
    let face_a = a.support_face(normal);
    if a.planes[face_a].normal.dot(normal) > FLAT {
        let vertex = b.support_vertex(-frame.rotate_inverse(normal));
        if let Some((manifold, separation)) = clip_faces(a, b, frame, face_a, vertex, contact) {
            cache.store(AxisKind::FaceA, separation, face_a, vertex);
            return Some(manifold);
        }
    }
    let local = -frame.rotate_inverse(normal);
    let face_b = b.support_face(local);
    if b.planes[face_b].normal.dot(local) > FLAT {
        let vertex = a.support_vertex(normal);
        if let Some((manifold, separation)) = face_b_contact(a, b, frame, vertex, face_b, contact) {
            cache.store(AxisKind::FaceB, separation, vertex, face_b);
            return Some(manifold);
        }
    }
    cache.reset();
    let surface_a = output.point_a + contact.radius_a * normal;
    let surface_b = output.point_b - contact.radius_b * normal;
    let mut manifold = Manifold {
        normal,
        ..Manifold::default()
    };
    manifold.push(point(
        surface_a.lerp(surface_b, 0.5),
        output.distance - contact.radius(),
        FeaturePair::SINGLE,
    ));
    Some(manifold)
}

pub(crate) fn collide_polyhedra(
    a: &Polyhedron<'_>,
    b: &Polyhedron<'_>,
    transform: Transform,
    contact: Contact,
    caches: &mut PairCache,
    find_axes: impl Fn(bool) -> AxisQuery,
) -> Manifold {
    let PairCache {
        simplex,
        sat: cache,
    } = caches;
    let frame = Frame::from_transform(transform);
    if contact.radius() > 0.0
        && let Some(manifold) = rounded_separated(a, b, transform, frame, contact, simplex, cache)
    {
        return manifold;
    }
    if let Some(manifold) = cached(a, b, frame, contact, cache) {
        return manifold;
    }
    cache.reset();

    let query = find_axes(true);
    if query.separated != AxisKind::None {
        let axis = match query.separated {
            AxisKind::FaceA => query.face_a,
            AxisKind::FaceB => query.face_b,
            _ => query.edge,
        };
        cache.store(
            query.separated,
            axis.separation - contact.radius(),
            axis.index_a,
            axis.index_b,
        );
        return Manifold::default();
    }

    let face = best_face(&query);
    let mut manifold = Manifold::default();
    if face.is_valid() {
        let built = match face.kind {
            AxisKind::FaceA => clip_faces(a, b, frame, face.index_a, face.index_b, contact),
            _ => face_b_contact(a, b, frame, face.index_a, face.index_b, contact),
        };
        if let Some((built, separation)) = built {
            manifold = built;
            cache.store(face.kind, separation, face.index_a, face.index_b);
        }
    }

    let mut edge = query.edge;
    if manifold.is_empty() && edge.separation < query.face_a.separation.max(query.face_b.separation)
    {
        edge = find_axes(false).edge;
    }
    if !edge.is_valid() {
        return manifold;
    }

    if (manifold.is_empty()
        || edge.separation - contact.radius() > cache.separation + contact.linear_slop)
        && let Some((edge_manifold, separation)) = edge_contact(
            a,
            b,
            frame,
            edge.normal,
            edge.index_a,
            edge.index_b,
            contact,
        )
    {
        cache.store(AxisKind::EdgePair, separation, edge.index_a, edge.index_b);
        return edge_manifold;
    }
    manifold
}
