use rikka_mesh_edit::{EditMesh, SelectMode};
use rikka_mesh_reduce::{
    Delimit, DissolveDegenerateOptions, DissolveEdgesOptions, DissolveFacesOptions,
    DissolveModeOptions, DissolveVertsOptions, LimitedDissolveOptions, dissolve_degenerate,
    dissolve_edges, dissolve_faces, dissolve_limited, dissolve_mode, dissolve_verts,
};

use crate::oracle::delete_dissolve as oracle;
use crate::support::{
    Mesh, Outcome, Pick, all_verts, assert_outcome, edge_list, edit_mesh, select,
};

fn prepared(fixture: &Mesh, mode: SelectMode, pick: Pick) -> EditMesh {
    let mut mesh = edit_mesh(fixture, mode);
    select(&mut mesh, pick);
    mesh
}

fn everything(fixture: &Mesh, mode: SelectMode) -> EditMesh {
    let mut mesh = edit_mesh(fixture, mode);
    let verts: &'static [u32] = Vec::leak(all_verts(&mesh));
    select(
        &mut mesh,
        Pick {
            verts,
            ..Pick::default()
        },
    );
    mesh
}

#[test]
fn dissolve_verts_matches_blender() {
    let cases: [(&'static [u32], DissolveVertsOptions, &Outcome); 4] = [
        (
            &[6, 8],
            DissolveVertsOptions::default(),
            &oracle::DISSOLVE_VERTS,
        ),
        (
            &[6, 8],
            DissolveVertsOptions {
                use_face_split: true,
                use_boundary_tear: false,
            },
            &oracle::DISSOLVE_VERTS_SPLIT,
        ),
        (
            &[2, 7],
            DissolveVertsOptions {
                use_face_split: false,
                use_boundary_tear: true,
            },
            &oracle::DISSOLVE_VERTS_TEAR,
        ),
        (
            &[2, 7, 13],
            DissolveVertsOptions {
                use_face_split: true,
                use_boundary_tear: true,
            },
            &oracle::DISSOLVE_VERTS_SPLIT_TEAR,
        ),
    ];
    for (verts, options, expected) in cases {
        let mut mesh = prepared(
            &oracle::GRID,
            SelectMode::VERT,
            Pick {
                verts,
                ..Pick::default()
            },
        );
        dissolve_verts(&mut mesh, &options).expect("operator runs");
        assert_outcome(&mesh, expected);
    }
}

#[test]
fn dissolve_edges_matches_blender() {
    let loop_edges: &'static [u32] = &[5, 16, 25];
    let cases: [(&Mesh, &'static [u32], DissolveEdgesOptions, &Outcome); 7] = [
        (
            &oracle::GRID,
            loop_edges,
            DissolveEdgesOptions::default(),
            &oracle::DISSOLVE_EDGES,
        ),
        (
            &oracle::GRID,
            loop_edges,
            DissolveEdgesOptions {
                use_verts: false,
                ..DissolveEdgesOptions::default()
            },
            &oracle::DISSOLVE_EDGES_KEEP_VERTS,
        ),
        (
            &oracle::GRID,
            &[16],
            DissolveEdgesOptions {
                use_face_split: true,
                ..DissolveEdgesOptions::default()
            },
            &oracle::DISSOLVE_EDGES_SPLIT,
        ),
        (
            &oracle::BENT,
            loop_edges,
            DissolveEdgesOptions {
                angle_threshold: 30.0f32.to_radians(),
                ..DissolveEdgesOptions::default()
            },
            &oracle::DISSOLVE_EDGES_ANGLE,
        ),
        (
            &oracle::TRIANGLES,
            &[17, 20, 21],
            DissolveEdgesOptions::default(),
            &oracle::DISSOLVE_EDGES_QUADS,
        ),
        (
            &oracle::TRIANGLES,
            &[17, 20, 21],
            DissolveEdgesOptions {
                use_preserve_quads: false,
                ..DissolveEdgesOptions::default()
            },
            &oracle::DISSOLVE_EDGES_NO_QUADS,
        ),
        (
            &oracle::GRID,
            &[2, 14, 23],
            DissolveEdgesOptions::default(),
            &oracle::DISSOLVE_MODE_EDGE,
        ),
    ];
    for (fixture, edges, options, expected) in cases {
        let mut mesh = prepared(
            fixture,
            SelectMode::EDGE,
            Pick {
                edges,
                ..Pick::default()
            },
        );
        dissolve_edges(&mut mesh, &options).expect("operator runs");
        assert_outcome(&mesh, expected);
    }
}

#[test]
fn dissolve_faces_matches_blender() {
    let cases: [(&'static [u32], bool, &Outcome); 3] = [
        (&[5, 6, 9, 10], false, &oracle::DISSOLVE_FACES),
        (&[5, 6, 9, 10], true, &oracle::DISSOLVE_FACES_VERTS),
        (
            &[0, 4, 3, 7, 11],
            false,
            &oracle::DISSOLVE_FACES_TWO_REGIONS,
        ),
    ];
    for (faces, use_verts, expected) in cases {
        let mut mesh = prepared(
            &oracle::GRID,
            SelectMode::FACE,
            Pick {
                faces,
                ..Pick::default()
            },
        );
        dissolve_faces(&mut mesh, &DissolveFacesOptions { use_verts }).expect("operator runs");
        assert_outcome(&mesh, expected);
    }
}

#[test]
fn dissolve_mode_follows_select_mode() {
    let mut mesh = prepared(
        &oracle::GRID,
        SelectMode::EDGE,
        Pick {
            edges: &[2, 14, 23],
            ..Pick::default()
        },
    );
    dissolve_mode(&mut mesh, &DissolveModeOptions::default()).expect("operator runs");
    assert_outcome(&mesh, &oracle::DISSOLVE_MODE_EDGE);

    let mut mesh = prepared(
        &oracle::GRID,
        SelectMode::FACE,
        Pick {
            faces: &[1, 2],
            ..Pick::default()
        },
    );
    dissolve_mode(&mut mesh, &DissolveModeOptions::default()).expect("operator runs");
    assert_outcome(&mesh, &oracle::DISSOLVE_MODE_FACE);
}

#[test]
fn limited_dissolve_matches_blender() {
    let cases: [(LimitedDissolveOptions, &Outcome); 3] = [
        (LimitedDissolveOptions::default(), &oracle::LIMITED),
        (
            LimitedDissolveOptions {
                use_dissolve_boundaries: true,
                ..LimitedDissolveOptions::default()
            },
            &oracle::LIMITED_BOUNDARIES,
        ),
        (
            LimitedDissolveOptions {
                angle_limit: 40.0f32.to_radians(),
                delimit: Delimit::UV,
                ..LimitedDissolveOptions::default()
            },
            &oracle::LIMITED_WIDE,
        ),
    ];
    for (options, expected) in cases {
        let mut mesh = everything(&oracle::WAVY, SelectMode::VERT);
        dissolve_limited(&mut mesh, &options).expect("operator runs");
        assert_outcome(&mesh, expected);
    }
}

#[test]
fn limited_dissolve_in_face_mode_keeps_partial_borders() {
    let mut mesh = prepared(
        &oracle::WAVY,
        SelectMode::FACE,
        Pick {
            faces: &[0, 1, 2, 3, 4, 5, 6, 7],
            ..Pick::default()
        },
    );
    dissolve_limited(&mut mesh, &LimitedDissolveOptions::default()).expect("operator runs");
    assert_outcome(&mesh, &oracle::LIMITED_FACE_MODE);
}

#[test]
fn limited_dissolve_respects_seams_and_sharp_edges() {
    let mut mesh = everything(&oracle::WAVY, SelectMode::VERT);
    let edges: Vec<_> = mesh.edges().collect();
    for (edge, pair) in edges.into_iter().zip(edge_list(&mesh)) {
        let sorted = [pair[0].min(pair[1]), pair[0].max(pair[1])];
        if [[6, 7], [7, 8], [11, 12]].contains(&sorted) {
            mesh.set_edge_seam(edge, true);
        }
        if sorted == [1, 6] {
            mesh.set_edge_sharp(edge, true);
        }
    }
    dissolve_limited(
        &mut mesh,
        &LimitedDissolveOptions {
            delimit: Delimit::SEAM | Delimit::SHARP,
            ..LimitedDissolveOptions::default()
        },
    )
    .expect("operator runs");
    assert_outcome(&mesh, &oracle::LIMITED_SEAM_SHARP);
}

#[test]
fn dissolve_degenerate_matches_blender() {
    for (fixture, expected) in [
        (&oracle::DEGENERATE_GRID, &oracle::DEGENERATE),
        (&oracle::SLIVER, &oracle::DEGENERATE_EAR),
    ] {
        let mut mesh = everything(fixture, SelectMode::VERT);
        dissolve_degenerate(&mut mesh, &DissolveDegenerateOptions::default())
            .expect("valid threshold");
        assert_outcome(&mesh, expected);
    }
}
