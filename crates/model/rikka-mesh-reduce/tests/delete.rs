use rikka_mesh_edit::SelectMode;
use rikka_mesh_reduce::{
    DeleteEdgeLoopOptions, DeleteLooseOptions, DeleteOptions, DeleteType, delete, delete_edge_loop,
    delete_loose,
};

use crate::oracle::delete_dissolve as oracle;
use crate::support::{Outcome, Pick, all_verts, assert_outcome, edit_mesh, select};

#[test]
fn delete_types_match_blender() {
    let cases: [(DeleteType, &'static [u32], &Outcome); 5] = [
        (DeleteType::Verts, &[6, 7, 12], &oracle::DELETE_VERT),
        (DeleteType::Edges, &[6, 7], &oracle::DELETE_EDGE),
        (DeleteType::Faces, &[6, 7, 11, 12], &oracle::DELETE_FACE),
        (
            DeleteType::EdgesFaces,
            &[6, 7, 8],
            &oracle::DELETE_EDGE_FACE,
        ),
        (
            DeleteType::OnlyFaces,
            &[6, 7, 8, 11, 12, 13],
            &oracle::DELETE_ONLY_FACE,
        ),
    ];
    for (kind, verts, expected) in cases {
        let mut mesh = edit_mesh(&oracle::GRID, SelectMode::VERT);
        select(
            &mut mesh,
            Pick {
                verts,
                ..Pick::default()
            },
        );
        delete(&mut mesh, &DeleteOptions { kind }).expect("operator runs");
        assert_outcome(&mesh, expected);
    }
}

#[test]
fn delete_without_selection_keeps_mesh() {
    let mut mesh = edit_mesh(&oracle::GRID, SelectMode::VERT);
    mesh.deselect_all();
    let removed = delete(
        &mut mesh,
        &DeleteOptions {
            kind: DeleteType::Faces,
        },
    )
    .expect("operator runs");
    assert_eq!(removed.faces, 0);
    assert_eq!(mesh.face_count(), 12);
}

#[test]
fn delete_loose_matches_blender() {
    for (options, expected) in [
        (
            DeleteLooseOptions {
                use_verts: true,
                use_edges: true,
                use_faces: true,
            },
            &oracle::DELETE_LOOSE,
        ),
        (DeleteLooseOptions::default(), &oracle::DELETE_LOOSE_DEFAULT),
    ] {
        let mut mesh = edit_mesh(&oracle::LOOSE, SelectMode::VERT);
        let verts: &'static [u32] = Vec::leak(all_verts(&mesh));
        select(
            &mut mesh,
            Pick {
                verts,
                ..Pick::default()
            },
        );
        delete_loose(&mut mesh, &options).expect("operator runs");
        assert_outcome(&mesh, expected);
    }
}

#[test]
fn delete_edge_loop_matches_blender() {
    for (use_face_split, expected) in [
        (true, &oracle::DELETE_EDGE_LOOP_SPLIT),
        (false, &oracle::DELETE_EDGE_LOOP),
    ] {
        let mut mesh = edit_mesh(&oracle::GRID, SelectMode::EDGE);
        select(
            &mut mesh,
            Pick {
                edges: &[5, 16, 25],
                ..Pick::default()
            },
        );
        delete_edge_loop(&mut mesh, &DeleteEdgeLoopOptions { use_face_split })
            .expect("operator runs");
        assert_outcome(&mesh, expected);
    }
}
