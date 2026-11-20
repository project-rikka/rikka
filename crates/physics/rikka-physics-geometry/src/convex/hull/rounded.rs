use std::f32::consts::TAU;

use rikka_physics_math::{Mat3, Vec3, atan2};

use super::view::Polyhedron;
use crate::MassData;

#[derive(Clone, Copy, Debug)]
struct Moments {
    volume: f32,
    first: Vec3,
    second: Mat3,
}

#[inline]
fn outer(a: Vec3, b: Vec3) -> Mat3 {
    Mat3::from_cols(a * b.x, a * b.y, a * b.z)
}

#[inline]
fn symmetric(a: Vec3, b: Vec3) -> Mat3 {
    outer(a, b) + outer(b, a)
}

#[inline]
fn angle_between(a: Vec3, b: Vec3) -> f32 {
    atan2(a.cross(b).length(), a.dot(b))
}

impl Moments {
    fn add(&mut self, volume: f32, first: Vec3, second: Mat3) {
        self.volume += volume;
        self.first += first;
        self.second += second;
    }

    fn faces(&mut self, hull: &Polyhedron<'_>, origin: Vec3, radius: f32) {
        for face in 0..hull.face_count() {
            let normal = hull.planes[face].normal;
            let mut ring = hull
                .face_edges(face)
                .map(|edge| hull.points[hull.edges[edge].origin as usize] - origin);
            let Some(anchor) = ring.next() else {
                continue;
            };
            let mut previous = ring.next().unwrap_or(anchor);
            let (mut area, mut first, mut second) = (0.0, Vec3::ZERO, Mat3::ZERO);
            for current in ring {
                let triangle = 0.5 * (previous - anchor).cross(current - anchor).length();
                let sum = anchor + previous + current;
                area += triangle;
                first += triangle / 3.0 * sum;
                second += (outer(anchor, anchor)
                    + outer(previous, previous)
                    + outer(current, current)
                    + outer(sum, sum))
                    * (triangle / 12.0);
                previous = current;
            }
            self.add(
                radius * area,
                radius * first + 0.5 * radius * radius * area * normal,
                second * radius
                    + symmetric(first, normal) * (0.5 * radius * radius)
                    + outer(normal, normal) * (radius.powi(3) / 3.0 * area),
            );
        }
    }

    fn edges(&mut self, hull: &Polyhedron<'_>, origin: Vec3, radius: f32) {
        for edge in (0..hull.edges.len()).step_by(2) {
            let (p, q) = hull.edge_points(edge);
            let (start, end) = (p - origin, q - origin);
            let u = hull.planes[hull.edges[edge].face as usize].normal;
            let v = hull.planes[hull.edges[edge + 1].face as usize].normal;
            let theta = angle_between(u, v);
            let w = (v - v.dot(u) * u).normalize_or_zero();
            let length = start.distance(end);
            if theta <= 0.0 || w == Vec3::ZERO || length <= 0.0 {
                continue;
            }
            let d = (end - start) / length;
            let arc_first = theta.sin() * u + (1.0 - theta.cos()) * w;
            let double = 2.0 * theta;
            let arc_second = outer(u, u) * (0.5 * theta + 0.25 * double.sin())
                + outer(w, w) * (0.5 * theta - 0.25 * double.sin())
                + symmetric(u, w) * (0.25 * (1.0 - double.cos()));
            let line_first = length * start + 0.5 * length * length * d;
            let line_second = outer(start, start) * length
                + symmetric(start, d) * (0.5 * length * length)
                + outer(d, d) * (length.powi(3) / 3.0);
            let sector = 0.5 * theta * radius * radius;
            self.add(
                sector * length,
                sector * line_first + radius.powi(3) / 3.0 * length * arc_first,
                line_second * sector
                    + symmetric(line_first, arc_first) * (radius.powi(3) / 3.0)
                    + arc_second * (radius.powi(4) / 4.0 * length),
            );
        }
    }

    fn vertices(&mut self, hull: &Polyhedron<'_>, origin: Vec3, radius: f32) {
        let mut corner_angles = [0.0f32; 256];
        for face in 0..hull.face_count() {
            let mut ring = [0u8; 256];
            let mut count = 0;
            for edge in hull.face_edges(face) {
                ring[count] = hull.edges[edge].origin;
                count += 1;
            }
            for index in 0..count {
                let point = |slot: usize| hull.points[ring[slot % count] as usize];
                let corner = point(index);
                let turn =
                    angle_between(point(index + count - 1) - corner, point(index + 1) - corner);
                corner_angles[ring[index] as usize] += turn;
            }
        }
        let cube = radius.powi(3) / 3.0;
        for (vertex, &corner) in corner_angles[..hull.vertex_count()].iter().enumerate() {
            let solid_angle = TAU - corner;
            let arc = cone_moment(hull, vertex);
            let point = hull.points[vertex] - origin;
            self.add(
                solid_angle * cube,
                solid_angle * cube * point + radius.powi(4) / 4.0 * arc,
                outer(point, point) * (solid_angle * cube)
                    + symmetric(point, arc) * (radius.powi(4) / 4.0)
                    + Mat3::IDENTITY * (radius.powi(5) / 15.0 * solid_angle),
            );
        }
    }
}

fn cone_moment(hull: &Polyhedron<'_>, vertex: usize) -> Vec3 {
    let start = hull.vertex_edges[vertex] as usize;
    let mut normals = [Vec3::ZERO; 256];
    let mut count = 0;
    let mut edge = start;
    loop {
        normals[count] = hull.planes[hull.edges[edge].face as usize].normal;
        count += 1;
        edge = hull.edges[hull.edges[edge].twin as usize].next as usize;
        if edge == start || count == normals.len() {
            break;
        }
    }
    let center = normals[..count]
        .iter()
        .copied()
        .sum::<Vec3>()
        .normalize_or_zero();
    let mut moment = Vec3::ZERO;
    for index in 0..count {
        let a = normals[index];
        let b = normals[(index + 1) % count];
        let mut plane = a.cross(b).normalize_or_zero();
        if plane.dot(center) < 0.0 {
            plane = -plane;
        }
        moment += 0.5 * angle_between(a, b) * plane;
    }
    moment
}

pub(crate) fn rounded_mass(
    hull: &Polyhedron<'_>,
    core: MassData,
    radius: f32,
    density: f32,
) -> MassData {
    let origin = core.center;
    let unit = core.inertia;
    let trace = unit.x_axis.x + unit.y_axis.y + unit.z_axis.z;
    let volume = core.mass;
    let mut moments = Moments {
        volume,
        first: Vec3::ZERO,
        second: Mat3::IDENTITY * (0.5 * trace) - unit,
    };
    moments.faces(hull, origin, radius);
    moments.edges(hull, origin, radius);
    moments.vertices(hull, origin, radius);

    let shift = moments.first / moments.volume;
    let central = moments.second - outer(moments.first, shift);
    let trace = central.x_axis.x + central.y_axis.y + central.z_axis.z;
    MassData {
        mass: density * moments.volume,
        center: origin + shift,
        inertia: (Mat3::IDENTITY * trace - central) * density,
    }
}
