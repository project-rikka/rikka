use std::collections::HashMap;

use glam::{DVec3, EulerRot, Mat4, Vec3};
use rikka_mesh_edit::{EditMesh, Primitive};
use rikka_polymesh::Polymesh;

pub(crate) fn primitive(primitive: &Primitive, matrix: Mat4) -> Polymesh {
    let mut mesh = EditMesh::new();
    mesh.add_primitive(primitive, matrix, true).unwrap();
    mesh.to_polymesh()
}

pub(crate) fn cube() -> Polymesh {
    primitive(&Primitive::cube(), Mat4::IDENTITY)
}

pub(crate) fn listed_cube() -> Polymesh {
    let positions = [
        [-1.0, -1.0, -1.0],
        [1.0, -1.0, -1.0],
        [1.0, 1.0, -1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0],
        [1.0, -1.0, 1.0],
        [1.0, 1.0, 1.0],
        [-1.0, 1.0, 1.0],
    ];
    Polymesh::from_polygons(
        positions.iter().map(|&p| Vec3::from_array(p)).collect(),
        [
            [0u32, 3, 2, 1],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [1, 2, 6, 5],
            [2, 3, 7, 6],
            [3, 0, 4, 7],
        ],
    )
}

pub(crate) fn uv_sphere() -> Polymesh {
    primitive(
        &Primitive::UvSphere {
            segments: 32,
            rings: 16,
            radius: 1.0,
        },
        Mat4::IDENTITY,
    )
}

pub(crate) fn torus() -> Polymesh {
    primitive(
        &Primitive::Torus {
            major_segments: 48,
            minor_segments: 12,
            major_radius: 1.0,
            minor_radius: 0.25,
        },
        Mat4::IDENTITY,
    )
}

pub(crate) fn grid() -> Polymesh {
    primitive(
        &Primitive::Grid {
            x_subdivisions: 4,
            y_subdivisions: 4,
            size: 3.0,
        },
        Mat4::IDENTITY,
    )
}

pub(crate) fn placement(location: [f32; 3], rotation: [f32; 3], scale: f32) -> Mat4 {
    Mat4::from_translation(Vec3::from_array(location))
        * Mat4::from_euler(EulerRot::ZYX, rotation[2], rotation[1], rotation[0])
        * Mat4::from_scale(Vec3::splat(scale))
}

pub(crate) fn moved(location: [f32; 3]) -> Mat4 {
    placement(location, [0.0; 3], 1.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Stats {
    pub(crate) verts: usize,
    pub(crate) edges: usize,
    pub(crate) faces: usize,
    pub(crate) volume: f64,
    pub(crate) area: f64,
    pub(crate) manifold: bool,
    pub(crate) quads: usize,
    pub(crate) tris: usize,
    pub(crate) ngons: usize,
}

pub(crate) fn stats(mesh: &Polymesh) -> Stats {
    let mut volume = 0.0;
    let mut area = 0.0;
    let mut quads = 0;
    let mut tris = 0;
    let mut ngons = 0;
    for face in 0..mesh.face_count() {
        let points: Vec<DVec3> = mesh
            .face_verts(face)
            .iter()
            .map(|&v| mesh.positions[v as usize].as_dvec3())
            .collect();
        match points.len() {
            3 => tris += 1,
            4 => quads += 1,
            _ => ngons += 1,
        }
        let mut normal = DVec3::ZERO;
        for i in 1..points.len() - 1 {
            normal += (points[i] - points[0]).cross(points[i + 1] - points[0]);
            volume += points[0].dot(points[i].cross(points[i + 1])) / 6.0;
        }
        area += normal.length() * 0.5;
    }
    let mut uses: HashMap<(u32, u32), usize> = HashMap::new();
    for face in 0..mesh.face_count() {
        let verts = mesh.face_verts(face);
        for i in 0..verts.len() {
            let (a, b) = (verts[i], verts[(i + 1) % verts.len()]);
            *uses.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    let manifold = mesh
        .edges
        .iter()
        .all(|edge| uses.get(&(edge[0].min(edge[1]), edge[0].max(edge[1]))) == Some(&2));
    Stats {
        verts: mesh.point_count(),
        edges: mesh.edge_count(),
        faces: mesh.face_count(),
        volume,
        area,
        manifold,
        quads,
        tris,
        ngons,
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Expected {
    pub(crate) verts: usize,
    pub(crate) edges: usize,
    pub(crate) faces: usize,
    pub(crate) volume: f64,
    pub(crate) area: f64,
    pub(crate) manifold: bool,
    pub(crate) quads: usize,
}

pub(crate) fn expect(
    verts: usize,
    edges: usize,
    faces: usize,
    volume: f64,
    area: f64,
    manifold: bool,
    quads: usize,
) -> Expected {
    Expected {
        verts,
        edges,
        faces,
        volume,
        area,
        manifold,
        quads,
    }
}

pub(crate) fn assert_stats(mesh: &Polymesh, expected: Expected, label: &str) {
    mesh.validate()
        .unwrap_or_else(|error| panic!("{label}: invalid mesh: {error}"));
    let actual = stats(mesh);
    let close = |a: f64, b: f64| (a - b).abs() <= 1.0e-4 * b.abs().max(1.0);
    assert!(
        actual.verts == expected.verts
            && actual.edges == expected.edges
            && actual.faces == expected.faces
            && close(actual.volume, expected.volume)
            && close(actual.area, expected.area)
            && actual.manifold == expected.manifold
            && actual.quads == expected.quads,
        "{label}: got {actual:?}, expected {expected:?}"
    );
}

pub(crate) fn assert_shape(mesh: &Polymesh, volume: f64, area: f64, manifold: bool, label: &str) {
    mesh.validate()
        .unwrap_or_else(|error| panic!("{label}: invalid mesh: {error}"));
    let actual = stats(mesh);
    let close = |a: f64, b: f64| (a - b).abs() <= 1.0e-4 * b.abs().max(1.0);
    assert!(
        close(actual.volume, volume) && close(actual.area, area) && actual.manifold == manifold,
        "{label}: got {actual:?}, expected volume {volume} area {area} manifold {manifold}"
    );
}
