use glam::{Vec2, Vec3};
use rikka_polymesh::{Polymesh, QuadMethod};
use rustc_hash::FxHashSet;

use crate::kinds::triangulate::Triangulate;
use crate::normals;

pub(crate) fn evaluate(triangulate: &Triangulate, mesh: &mut Polymesh) {
    let carried = match triangulate.keep_custom_normals {
        true => {
            let normals = mesh.corner_normals();
            mesh.custom_normals_mut().copy_from_slice(&normals);
            false
        }
        false => normals::encode_custom(mesh),
    };
    split(triangulate, mesh);
    if carried {
        normals::decode_custom(mesh);
    }
}

fn split(triangulate: &Triangulate, mesh: &mut Polymesh) {
    let key = |mesh: &Polymesh, corners: [u32; 3]| {
        let mut points = corners.map(|corner| mesh.corner_verts[corner as usize]);
        points.sort_unstable();
        points
    };
    let mut seen: FxHashSet<[u32; 3]> = (0..mesh.face_count())
        .map(|face| mesh.face(face))
        .filter(|corners| corners.len() == 3)
        .map(|corners| {
            let start = corners.start as u32;
            key(mesh, [start, start + 1, start + 2])
        })
        .collect();
    let min_vertices = triangulate.min_vertices.max(4) as usize;
    let mut faces: Vec<(u32, Vec<u32>)> = Vec::with_capacity(mesh.face_count());
    let mut appended: Vec<(u32, Vec<u32>)> = Vec::new();
    for face in 0..mesh.face_count() {
        let range = mesh.face(face);
        let corners: Vec<u32> = (range.start as u32..range.end as u32).collect();
        if corners.len() < min_vertices {
            faces.push((face as u32, corners));
            continue;
        }
        let triangles = match corners.len() {
            4 => quad_split(mesh, &corners, triangulate.quad_method).to_vec(),
            _ => mesh.triangulate_face_with(face, triangulate.quad_method, triangulate.ngon_method),
        };
        let Some((last, created)) = triangles.split_last() else {
            continue;
        };
        for triangle in created {
            if seen.insert(key(mesh, *triangle)) {
                appended.push((face as u32, triangle.to_vec()));
            }
        }
        if seen.insert(key(mesh, *last)) {
            faces.push((face as u32, last.to_vec()));
        }
    }
    faces.extend(appended);
    mesh.rebuild_faces(
        faces
            .iter()
            .map(|(face, corners)| (*face, corners.as_slice())),
    );
}

fn quad_split(mesh: &Polymesh, corners: &[u32], method: QuadMethod) -> [[u32; 3]; 2] {
    let at = |slot: usize| mesh.positions[mesh.corner_verts[corners[slot] as usize] as usize];
    let split_24 = match method {
        QuadMethod::Fixed => true,
        QuadMethod::Alternate => false,
        QuadMethod::ShortEdge => {
            at(1).distance_squared(at(3)) - at(0).distance_squared(at(2)) > 0.0
        }
        QuadMethod::LongEdge => at(1).distance_squared(at(3)) - at(0).distance_squared(at(2)) < 0.0,
        QuadMethod::Beauty => {
            let flip = quad_flip(at(1), at(2), at(3), at(0));
            if flip & 1 != 0 {
                true
            } else if flip & 2 != 0 {
                false
            } else {
                rotate_beauty(at(1), at(2), at(3), at(0)) > 0.0
            }
        }
    };
    let start = match split_24 {
        true => 0,
        false => 1,
    };
    let loops: [u32; 4] = std::array::from_fn(|slot| corners[(start + slot) % 4]);
    [
        [loops[0], loops[1], loops[2]],
        [loops[0], loops[2], loops[3]],
    ]
}

fn quad_flip(v1: Vec3, v2: Vec3, v3: Vec3, v4: Vec3) -> u8 {
    let (d12, d23, d34, d41) = (v1 - v2, v2 - v3, v3 - v4, v4 - v1);
    let mut flip = 0;
    if d12.cross(d23).dot(d34.cross(d41)) < 0.0 {
        flip |= 1;
    }
    if d23.cross(d34).dot(d41.cross(d12)) < 0.0 {
        flip |= 2;
    }
    flip
}

fn cross_2d(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    (a.x - b.x) * (b.y - c.y) + (a.y - b.y) * (c.x - b.x)
}

fn rotate_beauty(v1: Vec3, v2: Vec3, v3: Vec3, v4: Vec3) -> f32 {
    const DEGENERATE: f32 = 1.0e-5;
    const ZERO_AREA: f32 = 1.0e-12;
    let cross = |a: Vec3, b: Vec3, c: Vec3| (a - b).cross(b - c);
    let sum = cross(v2, v3, v4) + cross(v2, v4, v1);
    let scale = sum.length();
    if scale * scale <= 1.0e-35 {
        return f32::MAX;
    }
    let normal = sum / scale;
    let (first, second) = plane_axes(normal);
    let flat = |point: Vec3| Vec2::new(first.dot(point), second.dot(point));
    let (p1, p2, p3, p4) = (flat(v1), flat(v2), flat(v3), flat(v4));
    let sign = |value: f32| {
        if value > DEGENERATE {
            1
        } else if value < -DEGENERATE {
            -1
        } else {
            0
        }
    };
    let area_234 = cross_2d(p2, p3, p4);
    let area_241 = cross_2d(p2, p4, p1);
    if sign(area_234 / scale) + sign(area_241 / scale) == 0 {
        return f32::MAX;
    }
    let area_123 = cross_2d(p1, p2, p3);
    let area_134 = cross_2d(p1, p3, p4);
    if (area_123 >= 0.0) != (area_134 >= 0.0) {
        return f32::MAX;
    }
    if area_123.abs() <= ZERO_AREA || area_134.abs() <= ZERO_AREA {
        return f32::MAX;
    }
    if (area_234 >= 0.0) != (area_241 >= 0.0) {
        return -f32::MAX;
    }
    if area_234.abs() <= ZERO_AREA || area_241.abs() <= ZERO_AREA {
        return -f32::MAX;
    }
    let (l12, l23, l34, l41) = (
        p1.distance(p2),
        p2.distance(p3),
        p3.distance(p4),
        p4.distance(p1),
    );
    let (l13, l24) = (p1.distance(p3), p2.distance(p4));
    let fac_24 = area_234.abs() / (l23 + l34 + l24) + area_241.abs() / (l41 + l12 + l24);
    let fac_13 = area_123.abs() / (l12 + l23 + l13) + area_134.abs() / (l34 + l41 + l13);
    fac_24 - fac_13
}

fn plane_axes(normal: Vec3) -> (Vec3, Vec3) {
    let flat = normal.x * normal.x + normal.y * normal.y;
    if flat > f32::EPSILON {
        let d = 1.0 / flat.sqrt();
        let first = Vec3::new(normal.y * d, -normal.x * d, 0.0);
        let second = Vec3::new(
            -normal.z * first.y,
            normal.z * first.x,
            normal.x * first.y - normal.y * first.x,
        );
        (first, second)
    } else {
        (
            Vec3::new(if normal.z < 0.0 { -1.0 } else { 1.0 }, 0.0, 0.0),
            Vec3::Y,
        )
    }
}
