use glam::{Mat4, Vec2, Vec3};
use rikka_mesh_edit::{CircleFill, EditMesh, Primitive};

use crate::oracle;
use crate::support::{TOLERANCE, checked, micro2, micro3};

fn added(primitive: Primitive) -> rikka_polymesh::Polymesh {
    let mut mesh = EditMesh::new();
    mesh.add_primitive(&primitive, Mat4::IDENTITY, true)
        .expect("primitive");
    checked(&mesh);
    mesh.to_polymesh()
}

fn assert_exact(mesh: &rikka_polymesh::Polymesh, expected: &oracle::Primitive, name: &str) {
    let faces: Vec<Vec<u32>> = (0..mesh.face_count())
        .map(|face| mesh.face_verts(face).to_vec())
        .collect();
    let expected_faces: Vec<Vec<u32>> = expected.faces.iter().map(|face| face.to_vec()).collect();
    assert_eq!(faces, expected_faces, "{name} faces");
    assert_eq!(
        mesh.positions.len(),
        expected.verts.len(),
        "{name} vertex count"
    );
    for (index, (position, expected)) in mesh.positions.iter().zip(expected.verts).enumerate() {
        assert!(
            position.distance(micro3(expected)) < TOLERANCE,
            "{name} vertex {index}: {position} != {expected:?}"
        );
    }
    if !expected.uvs.is_empty() {
        let uvs = mesh.active_uv_map().expect("uv map");
        for (index, (uv, expected)) in uvs.iter().zip(expected.uvs).enumerate() {
            assert!(
                uv.distance(micro2(expected)) < TOLERANCE,
                "{name} uv {index}: {uv} != {expected:?}"
            );
        }
    }
}

fn unmatched_corners(
    mesh: &rikka_polymesh::Polymesh,
    positions: &[[i32; 3]],
    faces: &[&[u32]],
    uvs: &[[i32; 2]],
) -> usize {
    let ours_uvs = mesh.active_uv_map().expect("uv map");
    let mut theirs: Vec<(Vec3, Vec2)> = faces
        .iter()
        .flat_map(|face| face.iter())
        .zip(uvs)
        .map(|(&vert, uv)| (micro3(&positions[vert as usize]), micro2(uv)))
        .collect();
    let mut unmatched = 0;
    for corner in 0..mesh.corner_count() {
        let position = mesh.positions[mesh.corner_verts[corner] as usize];
        let uv = ours_uvs[corner];
        match theirs.iter().position(|&(other, other_uv)| {
            other.distance(position) < TOLERANCE && other_uv.distance(uv) < TOLERANCE
        }) {
            Some(found) => {
                theirs.swap_remove(found);
            }
            None => unmatched += 1,
        }
    }
    unmatched + theirs.len()
}

#[test]
fn simple_primitives_match_blender_layout() {
    let cases = [
        (Primitive::Plane { size: 2.0 }, &oracle::PRIM_PLANE, "plane"),
        (
            Primitive::Grid {
                x_subdivisions: 3,
                y_subdivisions: 2,
                size: 2.0,
            },
            &oracle::PRIM_GRID,
            "grid",
        ),
        (Primitive::Cube { size: 2.0 }, &oracle::PRIM_CUBE, "cube"),
        (
            Primitive::Circle {
                vertices: 6,
                radius: 1.0,
                fill: CircleFill::Nothing,
            },
            &oracle::PRIM_CIRCLE_NONE,
            "circle",
        ),
        (
            Primitive::Circle {
                vertices: 6,
                radius: 1.0,
                fill: CircleFill::Ngon,
            },
            &oracle::PRIM_CIRCLE_NGON,
            "circle ngon",
        ),
        (
            Primitive::Circle {
                vertices: 6,
                radius: 1.0,
                fill: CircleFill::TriangleFan,
            },
            &oracle::PRIM_CIRCLE_FAN,
            "circle fan",
        ),
        (
            Primitive::Cylinder {
                vertices: 5,
                radius: 1.0,
                depth: 2.0,
                cap: CircleFill::Ngon,
            },
            &oracle::PRIM_CYL_NGON,
            "cylinder",
        ),
        (
            Primitive::Cylinder {
                vertices: 5,
                radius: 1.0,
                depth: 2.0,
                cap: CircleFill::Nothing,
            },
            &oracle::PRIM_CYL_NONE,
            "open cylinder",
        ),
        (
            Primitive::Cone {
                vertices: 5,
                radius_bottom: 1.0,
                radius_top: 0.0,
                depth: 2.0,
                cap: CircleFill::TriangleFan,
            },
            &oracle::PRIM_CONE_FAN,
            "cone fan",
        ),
        (
            Primitive::Cone {
                vertices: 5,
                radius_bottom: 1.0,
                radius_top: 0.0,
                depth: 2.0,
                cap: CircleFill::Ngon,
            },
            &oracle::PRIM_CONE_NGON,
            "cone",
        ),
        (
            Primitive::Cone {
                vertices: 5,
                radius_bottom: 1.0,
                radius_top: 0.0,
                depth: 2.0,
                cap: CircleFill::Nothing,
            },
            &oracle::PRIM_CONE_NONE,
            "open cone",
        ),
        (
            Primitive::Cone {
                vertices: 4,
                radius_bottom: 0.0,
                radius_top: 0.5,
                depth: 1.0,
                cap: CircleFill::Ngon,
            },
            &oracle::PRIM_CONE_INV,
            "inverted cone",
        ),
        (
            Primitive::IcoSphere {
                subdivisions: 1,
                radius: 1.0,
            },
            &oracle::PRIM_ICO1,
            "icosahedron",
        ),
    ];
    for (primitive, expected, name) in cases {
        assert_exact(&added(primitive), expected, name);
    }
}

#[test]
fn spheres_match_blender_geometry() {
    let cases = [
        (
            Primitive::UvSphere {
                segments: 5,
                rings: 4,
                radius: 1.0,
            },
            &oracle::PRIM_UVSPHERE,
        ),
        (
            Primitive::IcoSphere {
                subdivisions: 2,
                radius: 1.0,
            },
            &oracle::PRIM_ICO2,
        ),
        (
            Primitive::IcoSphere {
                subdivisions: 3,
                radius: 1.0,
            },
            &oracle::PRIM_ICO3,
        ),
    ];
    for (primitive, expected) in cases {
        let mesh = added(primitive);
        assert_eq!(mesh.point_count(), expected.verts.len());
        assert_eq!(
            unmatched_corners(&mesh, expected.verts, expected.faces, expected.uvs),
            0,
            "{primitive:?}"
        );
    }
}

#[test]
fn torus_follows_major_minor_layout() {
    let mesh = added(Primitive::Torus {
        major_segments: 6,
        minor_segments: 4,
        major_radius: 1.0,
        minor_radius: 0.25,
    });
    assert_eq!(mesh.point_count(), 24);
    assert_eq!(mesh.face_count(), 24);
    assert_eq!(mesh.face_verts(0), &[0, 4, 5, 1]);
    assert_eq!(mesh.face_verts(3), &[3, 7, 4, 0]);
    assert!((mesh.positions[0] - Vec3::new(1.25, 0.0, 0.0)).length() < TOLERANCE);
    assert!((mesh.positions[1] - Vec3::new(1.0, 0.0, 0.25)).length() < TOLERANCE);
    let uvs = mesh.active_uv_map().expect("uv map");
    assert!(uvs[0].distance(Vec2::new(0.5, 0.5)) < TOLERANCE);
    assert!(uvs[2].distance(Vec2::new(0.5 + 1.0 / 6.0, 0.75)) < TOLERANCE);
}

#[test]
fn primitive_respects_matrix() {
    let mut mesh = EditMesh::new();
    let matrix = Mat4::from_translation(Vec3::new(0.0, 0.0, 5.0));
    let added = mesh
        .add_primitive(&Primitive::cube(), matrix, false)
        .expect("cube");
    assert_eq!(added.verts.len(), 8);
    assert!(
        added
            .verts
            .iter()
            .all(|&vert| (mesh.position(vert).z - 5.0).abs() == 1.0)
    );
    assert!(mesh.uv_maps().is_empty());
}

#[test]
fn primitive_rejects_bad_parameters() {
    let mut mesh = EditMesh::new();
    assert!(
        mesh.add_primitive(
            &Primitive::Circle {
                vertices: 2,
                radius: 1.0,
                fill: CircleFill::Ngon
            },
            Mat4::IDENTITY,
            false
        )
        .is_err()
    );
    assert_eq!(mesh.vert_count(), 0);
}
