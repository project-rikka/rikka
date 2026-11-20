use std::f32::consts::PI;

use rikka_mesh_edit::{EditMesh, SelectMode};
use rikka_mesh_reduce::{JoinTrianglesOptions, join_triangles};

use crate::oracle::reduce as oracle;
use crate::support::{Outcome, Pick, all_verts, assert_outcome, edit_mesh, select};

fn faces(mesh: &mut EditMesh, faces: &'static [u32]) {
    select(
        mesh,
        Pick {
            faces,
            ..Pick::default()
        },
    );
}

const ALL_FACES: &[u32] = &[
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
];

#[test]
fn join_triangles_matches_blender() {
    let cases: [(JoinTrianglesOptions, &Outcome); 3] = [
        (JoinTrianglesOptions::default(), &oracle::JOIN),
        (
            JoinTrianglesOptions {
                face_threshold: Some(PI),
                shape_threshold: Some(PI),
                ..JoinTrianglesOptions::default()
            },
            &oracle::JOIN_LOOSE,
        ),
        (
            JoinTrianglesOptions {
                topology_influence: 1.5,
                face_threshold: Some(60.0f32.to_radians()),
                shape_threshold: Some(70.0f32.to_radians()),
                ..JoinTrianglesOptions::default()
            },
            &oracle::JOIN_TOPOLOGY,
        ),
    ];
    for (options, expected) in cases {
        let mut mesh = edit_mesh(&oracle::JOIN_FIXTURE, SelectMode::FACE);
        faces(&mut mesh, ALL_FACES);
        join_triangles(&mut mesh, &options).expect("operator runs");
        assert_outcome(&mesh, expected);
    }
}

#[test]
fn join_triangles_can_deselect_joined() {
    let mut mesh = edit_mesh(&oracle::JOIN_FIXTURE, SelectMode::VERT);
    let verts: &'static [u32] = Vec::leak(all_verts(&mesh));
    select(
        &mut mesh,
        Pick {
            verts,
            ..Pick::default()
        },
    );
    join_triangles(
        &mut mesh,
        &JoinTrianglesOptions {
            deselect_joined: true,
            ..JoinTrianglesOptions::default()
        },
    )
    .expect("operator runs");
    assert_outcome(&mesh, &oracle::JOIN_DESELECT);
}

#[test]
fn join_triangles_pair_ignores_thresholds() {
    let mut mesh = edit_mesh(&oracle::JOIN_FIXTURE, SelectMode::FACE);
    faces(&mut mesh, &[2, 14]);
    join_triangles(&mut mesh, &JoinTrianglesOptions::default()).expect("operator runs");
    assert_outcome(&mesh, &oracle::JOIN_PAIR);
}

#[test]
fn join_triangles_compares_attributes() {
    let mut mesh = edit_mesh(&oracle::JOIN_MIXED, SelectMode::FACE);
    let all_faces: Vec<_> = mesh.faces().collect();
    for (index, face) in all_faces.into_iter().enumerate() {
        if index % 5 == 0 {
            mesh.set_face_material(face, 1);
        }
    }
    let all_edges: Vec<_> = mesh.edges().collect();
    for (index, edge) in all_edges.into_iter().enumerate() {
        if index % 4 == 0 {
            mesh.set_edge_seam(edge, true);
        }
        if index % 6 == 1 {
            mesh.set_edge_sharp(edge, true);
        }
    }
    faces(&mut mesh, ALL_FACES);
    join_triangles(
        &mut mesh,
        &JoinTrianglesOptions {
            face_threshold: Some(PI),
            shape_threshold: Some(PI),
            uvs: true,
            seam: true,
            sharp: true,
            materials: true,
            ..JoinTrianglesOptions::default()
        },
    )
    .expect("operator runs");
    assert_outcome(&mesh, &oracle::JOIN_COMPARE);
}
