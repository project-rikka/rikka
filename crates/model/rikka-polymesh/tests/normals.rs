use std::f32::consts::{FRAC_1_SQRT_2, TAU};

use glam::Vec3;
use rikka_polymesh::{CUSTOM_NORMAL, Polymesh, Shading, polygon_normal};

use crate::support::{assert_close, cube, pair_set, points, prism, vec3};

const THIRD: f32 = 0.577_350_3;

#[test]
fn smooth_cube_matches_blender() {
    let mesh = cube();
    let expected_points = vec3(&[
        [-THIRD, -THIRD, -THIRD],
        [THIRD, -THIRD, -THIRD],
        [THIRD, THIRD, -THIRD],
        [-THIRD, THIRD, -THIRD],
        [-THIRD, -THIRD, THIRD],
        [THIRD, -THIRD, THIRD],
        [THIRD, THIRD, THIRD],
        [-THIRD, THIRD, THIRD],
    ]);
    assert_close(&mesh.point_normals(), &expected_points, 1.0e-5);
    let expected_corners: Vec<Vec3> = mesh
        .corner_verts
        .iter()
        .map(|&point| expected_points[point as usize])
        .collect();
    assert_close(&mesh.corner_normals(), &expected_corners, 1.0e-5);
    assert_close(
        &mesh.face_normals(),
        &vec3(&[
            [0.0, 0.0, -1.0],
            [0.0, 0.0, 1.0],
            [0.0, -1.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [-1.0, 0.0, 0.0],
        ]),
        1.0e-6,
    );
}

#[test]
fn non_planar_quad_normal_matches_blender() {
    let quad = points(&[
        [0.0, 0.0, 0.0],
        [2.0, 0.0, 0.7],
        [2.0, 2.0, 0.0],
        [0.0, 2.0, 0.9],
    ]);
    let normal = polygon_normal(&quad, &[0, 1, 2, 3]);
    assert!(
        normal.abs_diff_eq(Vec3::new(0.049_875, -0.049_875, 0.997_509), 1.0e-5),
        "{normal}"
    );
}

#[test]
fn sharp_edges_split_corner_normals_like_blender() {
    let mut mesh = cube();
    let rim = [[4, 5], [5, 6], [6, 7], [4, 7]];
    let sharp: Vec<bool> = mesh
        .edges
        .iter()
        .map(|edge| rim.contains(&rikka_polymesh::ordered(*edge)))
        .collect();
    mesh.sharp_edges_mut().copy_from_slice(&sharp);
    let h = FRAC_1_SQRT_2;
    let expected = vec3(&[
        [-THIRD, -THIRD, -THIRD],
        [-THIRD, THIRD, -THIRD],
        [THIRD, THIRD, -THIRD],
        [THIRD, -THIRD, -THIRD],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [-THIRD, -THIRD, -THIRD],
        [THIRD, -THIRD, -THIRD],
        [h, -h, 0.0],
        [-h, -h, 0.0],
        [THIRD, -THIRD, -THIRD],
        [THIRD, THIRD, -THIRD],
        [h, h, 0.0],
        [h, -h, 0.0],
        [THIRD, THIRD, -THIRD],
        [-THIRD, THIRD, -THIRD],
        [-h, h, 0.0],
        [h, h, 0.0],
        [-THIRD, THIRD, -THIRD],
        [-THIRD, -THIRD, -THIRD],
        [-h, -h, 0.0],
        [-h, h, 0.0],
    ]);
    assert_close(&mesh.corner_normals(), &expected, 1.0e-5);
}

fn sharpened_prism() -> Polymesh {
    let mut mesh = prism(16);
    mesh.sharpen_by_angle(30.0_f32.to_radians(), false);
    mesh
}

fn prism_corner_normals(mesh: &Polymesh) -> Vec<Vec3> {
    let side = |point: u32| {
        let angle = TAU * (point % 16) as f32 / 16.0;
        Vec3::new(angle.cos(), angle.sin(), 0.0)
    };
    (0..mesh.face_count())
        .flat_map(|face| {
            mesh.face_verts(face)
                .iter()
                .map(|&point| match face {
                    0 => Vec3::NEG_Z,
                    1 => Vec3::Z,
                    _ => side(point),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn smooth_by_angle_marks_cap_rims_like_blender() {
    let mesh = sharpened_prism();
    let sharp = mesh.sharp_edges().expect("sharp edges are written");
    let mut expected: Vec<[u32; 2]> = (0..16)
        .flat_map(|side| [[side, (side + 1) % 16], [16 + side, 16 + (side + 1) % 16]])
        .map(rikka_polymesh::ordered)
        .collect();
    expected.sort_unstable();
    assert_eq!(
        pair_set(&mesh, (0..mesh.edge_count()).filter(|&edge| sharp[edge])),
        expected
    );
    assert_close(&mesh.corner_normals(), &prism_corner_normals(&mesh), 1.0e-5);
}

#[test]
fn custom_normals_encode_like_blender() {
    let mut mesh = sharpened_prism();
    let requested: Vec<Vec3> = mesh
        .corner_normals()
        .iter()
        .map(|normal| (*normal + Vec3::new(0.0, 0.0, 0.3)).normalize())
        .collect();
    let sharp_before = mesh.sharp_edges().map(<[bool]>::to_vec);
    let encoded = mesh.encode_custom_normals(&requested);
    assert_eq!(mesh.sharp_edges().map(<[bool]>::to_vec), sharp_before);
    for (corner, data) in encoded.iter().enumerate() {
        let expected = match corner {
            0..32 => [0, 0],
            _ if (corner - 32) % 4 < 2 => [5611, 16384],
            _ => [5611, -16384],
        };
        assert!(
            (i32::from(data[0]) - expected[0]).abs() <= 1
                && (i32::from(data[1]) - expected[1]).abs() <= 4,
            "corner {corner}: {data:?} is not {expected:?}"
        );
    }
    let decoded = mesh.decode_custom_normals(&encoded);
    assert_close(
        &decoded[32..36],
        &vec3(&[
            [0.957_844, -1.4e-5, 0.287_29],
            [0.884_937, 0.366_538, 0.287_29],
            [0.884_927, 0.366_564, 0.287_29],
            [0.957_844, 1.4e-5, 0.287_29],
        ]),
        1.0e-4,
    );
    assert_close(&decoded, &requested, 2.0e-4);
}

#[test]
fn custom_normals_override_shading() {
    let mut mesh = cube();
    let up = vec![Vec3::new(0.0, 0.0, 2.0); mesh.corner_count()];
    mesh.custom_normals_mut().copy_from_slice(&up);
    assert!(mesh.attributes.contains(CUSTOM_NORMAL));
    for shading in [Shading::Split, Shading::Vertex] {
        assert!(mesh.shade(shading).iter().all(|normal| *normal == Vec3::Z));
    }
}

#[test]
fn vertex_shading_keeps_flat_faces_flat() {
    let mut mesh = cube();
    mesh.sharp_faces_mut()[1] = true;
    let normals = mesh.shade(Shading::Vertex);
    assert!(
        normals[4..8]
            .iter()
            .all(|normal| normal.abs_diff_eq(Vec3::Z, 1.0e-6))
    );
    assert!(normals[0].abs_diff_eq(Vec3::splat(-THIRD), 1.0e-5));
}
