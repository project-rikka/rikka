use glam::Vec3;
use rikka_mesh_edit::{EditMesh, IdMap, IdSet, SelectMode, VertId};
use rikka_mesh_reduce::{
    AutoMergeOptions, Error, MergeOptions, MergeType, RemoveDoublesOptions, WeldOptions,
    auto_merge, auto_merge_split, collapse_edges, merge, remove_doubles, weld,
};

use crate::oracle::merge as oracle;
use crate::support::{
    Outcome, Pick, all_verts, assert_mesh, assert_outcome, edit_mesh, select, vert,
};

fn weighted(mode: SelectMode, pick: Pick) -> EditMesh {
    let mut mesh = edit_mesh(&oracle::WEIGHTED, mode);
    select(&mut mesh, pick);
    mesh
}

fn picked() -> Pick {
    Pick {
        verts: &[6, 7, 12],
        history: &[6, 7, 12],
        ..Pick::default()
    }
}

#[test]
fn merge_types_match_blender() {
    let cases: [(MergeOptions, &Outcome); 5] = [
        (MergeOptions::default(), &oracle::MERGE_CENTER),
        (
            MergeOptions {
                uvs: true,
                ..MergeOptions::default()
            },
            &oracle::MERGE_CENTER_UVS,
        ),
        (
            MergeOptions {
                kind: MergeType::Cursor,
                uvs: true,
                cursor: Vec3::new(1.5, 1.25, 0.5),
            },
            &oracle::MERGE_CURSOR,
        ),
        (
            MergeOptions {
                kind: MergeType::First,
                uvs: true,
                ..MergeOptions::default()
            },
            &oracle::MERGE_FIRST,
        ),
        (
            MergeOptions {
                kind: MergeType::Last,
                ..MergeOptions::default()
            },
            &oracle::MERGE_LAST,
        ),
    ];
    for (options, expected) in cases {
        let mut mesh = weighted(SelectMode::VERT, picked());
        merge(&mut mesh, &options).expect("merge runs");
        assert_outcome(&mesh, expected);
    }
}

#[test]
fn merge_collapse_matches_blender() {
    let mut mesh = weighted(
        SelectMode::VERT,
        Pick {
            verts: &[6, 7, 13, 14],
            ..Pick::default()
        },
    );
    merge(
        &mut mesh,
        &MergeOptions {
            kind: MergeType::Collapse,
            uvs: true,
            ..MergeOptions::default()
        },
    )
    .expect("merge runs");
    assert_outcome(&mesh, &oracle::MERGE_COLLAPSE);
}

#[test]
fn merge_outside_vertex_mode_clears_selection() {
    let mut mesh = weighted(
        SelectMode::EDGE,
        Pick {
            edges: &[6, 16],
            ..Pick::default()
        },
    );
    merge(&mut mesh, &MergeOptions::default()).expect("merge runs");
    assert_outcome(&mesh, &oracle::MERGE_CENTER_EDGE_MODE);
}

#[test]
fn merge_at_first_needs_history_vertex() {
    let mut mesh = weighted(
        SelectMode::VERT,
        Pick {
            verts: &[6, 7],
            ..Pick::default()
        },
    );
    let result = merge(
        &mut mesh,
        &MergeOptions {
            kind: MergeType::First,
            ..MergeOptions::default()
        },
    );
    assert!(matches!(result, Err(Error::NoHistoryVertex)));
    assert_eq!(mesh.vert_count(), 20);
}

#[test]
fn edge_collapse_matches_blender() {
    let mut mesh = weighted(
        SelectMode::EDGE,
        Pick {
            edges: &[6, 16, 25, 3],
            ..Pick::default()
        },
    );
    collapse_edges(&mut mesh).expect("collapse runs");
    assert_outcome(&mesh, &oracle::EDGE_COLLAPSE);
}

fn doubles(mode: SelectMode, pick: Pick) -> EditMesh {
    let mut mesh = edit_mesh(&oracle::DOUBLES_FIXTURE, mode);
    select(&mut mesh, pick);
    mesh
}

fn all(mesh: &EditMesh) -> &'static [u32] {
    Vec::leak(all_verts(mesh))
}

#[test]
fn merge_by_distance_matches_blender() {
    let cases: [(RemoveDoublesOptions, &Outcome); 2] = [
        (
            RemoveDoublesOptions {
                threshold: 0.001,
                ..RemoveDoublesOptions::default()
            },
            &oracle::DOUBLES,
        ),
        (
            RemoveDoublesOptions {
                threshold: 0.001,
                use_centroid: false,
                ..RemoveDoublesOptions::default()
            },
            &oracle::DOUBLES_NO_CENTROID,
        ),
    ];
    for (options, expected) in cases {
        let mut mesh = edit_mesh(&oracle::DOUBLES_FIXTURE, SelectMode::VERT);
        let verts = all(&mesh);
        select(
            &mut mesh,
            Pick {
                verts,
                ..Pick::default()
            },
        );
        let removed = remove_doubles(&mut mesh, &options).expect("valid threshold");
        assert_eq!(removed.verts, 3);
        assert_outcome(&mesh, expected);
    }
}

#[test]
fn merge_by_distance_into_unselected_matches_blender() {
    let mut mesh = doubles(
        SelectMode::VERT,
        Pick {
            verts: &[4, 5, 6, 7],
            ..Pick::default()
        },
    );
    remove_doubles(
        &mut mesh,
        &RemoveDoublesOptions {
            threshold: 0.001,
            use_unselected: true,
            ..RemoveDoublesOptions::default()
        },
    )
    .expect("valid threshold");
    assert_outcome(&mesh, &oracle::DOUBLES_UNSELECTED);
}

#[test]
fn merge_by_distance_keeps_face_selection() {
    let mut mesh = doubles(
        SelectMode::FACE,
        Pick {
            faces: &[1, 0],
            ..Pick::default()
        },
    );
    remove_doubles(
        &mut mesh,
        &RemoveDoublesOptions {
            threshold: 0.001,
            ..RemoveDoublesOptions::default()
        },
    )
    .expect("valid threshold");
    assert_outcome(&mesh, &oracle::DOUBLES_FACE_MODE);
}

#[test]
fn merge_by_distance_splits_self_merged_face() {
    let mut mesh = edit_mesh(&oracle::SELF_MERGE, SelectMode::VERT);
    let verts = all(&mesh);
    select(
        &mut mesh,
        Pick {
            verts,
            ..Pick::default()
        },
    );
    remove_doubles(
        &mut mesh,
        &RemoveDoublesOptions {
            threshold: 0.001,
            ..RemoveDoublesOptions::default()
        },
    )
    .expect("valid threshold");
    assert_outcome(&mesh, &oracle::DOUBLES_SELF);
}

#[test]
fn merge_by_distance_rejects_negative_threshold() {
    let mut mesh = doubles(SelectMode::VERT, Pick::default());
    let result = remove_doubles(
        &mut mesh,
        &RemoveDoublesOptions {
            threshold: -1.0,
            ..RemoveDoublesOptions::default()
        },
    );
    assert!(matches!(result, Err(Error::OutOfRange("threshold"))));
}

#[test]
fn weld_matches_blender() {
    let mut mesh = edit_mesh(&oracle::DOUBLES_FIXTURE, SelectMode::VERT);
    let verts = all(&mesh);
    select(
        &mut mesh,
        Pick {
            verts,
            ..Pick::default()
        },
    );
    let targets: IdMap<VertId, VertId> = [(7, 2), (4, 1), (8, 1)]
        .into_iter()
        .map(|(from, to)| (vert(&mesh, from), vert(&mesh, to)))
        .collect();
    weld(&mut mesh, &targets, &WeldOptions::default()).expect("valid targets");
    assert_outcome(&mesh, &oracle::WELD);
}

#[test]
fn weld_rejects_chained_targets() {
    let mut mesh = doubles(SelectMode::VERT, Pick::default());
    let targets: IdMap<VertId, VertId> = [(7, 2), (2, 1)]
        .into_iter()
        .map(|(from, to)| (vert(&mesh, from), vert(&mesh, to)))
        .collect();
    let result = weld(&mut mesh, &targets, &WeldOptions::default());
    assert!(matches!(result, Err(Error::InvalidWeldTarget(_))));
    assert_eq!(mesh.vert_count(), 11);
}

fn moved(offset: Vec3) -> (EditMesh, IdSet<VertId>) {
    let mut mesh = edit_mesh(&oracle::AUTOMERGE_FIXTURE, SelectMode::VERT);
    select(
        &mut mesh,
        Pick {
            verts: &[6],
            ..Pick::default()
        },
    );
    let moved: IdSet<VertId> = [vert(&mesh, 6)].into_iter().collect();
    mesh.translate(moved.iter(), offset);
    mesh.update_normals();
    (mesh, moved)
}

const AUTOMERGE: AutoMergeOptions = AutoMergeOptions {
    threshold: 0.01,
    use_centroid: false,
};

#[test]
fn auto_merge_matches_blender() {
    for (offset, expected) in [
        (Vec3::new(0.995, 0.0, 0.0), &oracle::AUTOMERGE),
        (Vec3::new(0.503, -1.0, 0.0), &oracle::AUTOMERGE_VERT),
    ] {
        let (mut mesh, moved) = moved(offset);
        auto_merge(&mut mesh, &moved, &AUTOMERGE).expect("valid threshold");
        assert_outcome(&mesh, expected);
    }
}

#[test]
fn auto_merge_split_matches_blender() {
    for (offset, expected) in [
        (Vec3::new(1.003, -1.0, 0.0), &oracle::AUTOMERGE_SPLIT_VERT),
        (Vec3::new(1.5, -1.5, 0.0), &oracle::AUTOMERGE_SPLIT_EDGE),
    ] {
        let (mut mesh, moved) = moved(offset);
        auto_merge_split(&mut mesh, &moved, &AUTOMERGE).expect("valid threshold");
        assert_mesh(&mesh, &expected.mesh);
    }
}

#[test]
fn merge_by_distance_can_sharpen_custom_normal_splits() {
    for (use_sharp_edge_from_normals, sharpened) in [(false, 0), (true, 1)] {
        let mut mesh = edit_mesh(&oracle::WEIGHTED, SelectMode::VERT);
        let loops: Vec<_> = mesh.faces().map(|face| mesh.face_loops_vec(face)).collect();
        let normals = mesh.custom_normals_mut();
        for (face, corners) in loops.iter().enumerate() {
            let normal = match face % 4 < 2 {
                true => Vec3::Z,
                false => Vec3::X,
            };
            for lp in corners {
                normals[lp.index()] = normal;
            }
        }
        let verts = all(&mesh);
        select(
            &mut mesh,
            Pick {
                verts,
                ..Pick::default()
            },
        );
        let before = mesh
            .edges()
            .filter(|&edge| mesh.is_edge_sharp(edge))
            .count();
        remove_doubles(
            &mut mesh,
            &RemoveDoublesOptions {
                use_sharp_edge_from_normals,
                ..RemoveDoublesOptions::default()
            },
        )
        .expect("valid threshold");
        let after = mesh
            .edges()
            .filter(|&edge| mesh.is_edge_sharp(edge))
            .count();
        assert_eq!(after - before, sharpened * 3);
    }
}
