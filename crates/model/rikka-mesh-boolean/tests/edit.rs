use rikka_mesh_boolean::{
    IntersectBoolean, IntersectKnife, IntersectSource, Operation, SeparateMode, Solver,
};
use rikka_mesh_edit::{EditMesh, SelectMode};

use crate::support::{Expected, assert_stats, cube, expect, moved};

fn overlapping_cubes(mode: SelectMode, select_all: bool) -> EditMesh {
    let mut joined = cube();
    let mut second = cube();
    second.transform(&moved([1.0, 0.5, 0.25]));
    joined.append(&second);
    let mut mesh = EditMesh::from_polymesh(&joined).unwrap();
    mesh.set_select_mode(mode);
    let faces: Vec<_> = mesh.faces().collect();
    for (index, face) in faces.into_iter().enumerate() {
        if select_all || index >= 6 {
            mesh.select_face(face, true);
        }
    }
    mesh
}

fn selection(mesh: &EditMesh) -> [usize; 3] {
    [
        mesh.verts().filter(|&v| mesh.is_vert_selected(v)).count(),
        mesh.edges().filter(|&e| mesh.is_edge_selected(e)).count(),
        mesh.faces().filter(|&f| mesh.is_face_selected(f)).count(),
    ]
}

fn check(mesh: &EditMesh, expected: Expected, selected: [usize; 3], label: &str) {
    mesh.validate()
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    assert_stats(&mesh.to_polymesh(), expected, label);
    assert_eq!(selection(mesh), selected, "{label}: selection");
}

#[test]
fn intersect_knife_matches_blender() {
    let all = expect(40, 54, 18, 16.0, 48.0, false, 12);
    let merged = expect(22, 36, 18, 16.0, 48.0, false, 12);
    let cut = expect(28, 42, 18, 16.0, 48.0, true, 12);
    let cases = [
        (
            IntersectSource::SelectedUnselected,
            SeparateMode::All,
            all,
            [24, 24, 0],
        ),
        (
            IntersectSource::SelectedUnselected,
            SeparateMode::Cut,
            cut,
            [12, 12, 0],
        ),
        (
            IntersectSource::SelectedUnselected,
            SeparateMode::Merge,
            merged,
            [6, 6, 0],
        ),
        (
            IntersectSource::SelfIntersect,
            SeparateMode::All,
            all,
            [24, 24, 0],
        ),
        (
            IntersectSource::SelfIntersect,
            SeparateMode::Cut,
            all,
            [24, 24, 0],
        ),
        (
            IntersectSource::SelfIntersect,
            SeparateMode::Merge,
            merged,
            [6, 6, 0],
        ),
    ];
    for solver in [Solver::Exact, Solver::Fast] {
        for (source, separate, expected, selected) in cases {
            let mut mesh =
                overlapping_cubes(SelectMode::EDGE, source == IntersectSource::SelfIntersect);
            let changed = IntersectKnife::new()
                .source(source)
                .separate(separate)
                .solver(solver)
                .apply(&mut mesh)
                .unwrap();
            assert!(changed);
            check(
                &mesh,
                expected,
                selected,
                &format!("knife {solver:?} {source:?} {separate:?}"),
            );
        }
    }
}

#[test]
fn intersect_boolean_matches_blender() {
    let cases = [
        (
            Operation::Intersect,
            expect(8, 12, 6, 2.625, 11.75, true, 6),
        ),
        (Operation::Union, expect(20, 30, 12, 13.375, 36.25, true, 6)),
        (
            Operation::Difference,
            expect(14, 21, 9, 5.375, 24.0, true, 6),
        ),
    ];
    for solver in [Solver::Exact, Solver::Fast] {
        for (operation, expected) in cases {
            for swap in [false, true] {
                let mut mesh = overlapping_cubes(SelectMode::EDGE, false);
                let changed = IntersectBoolean::new(operation)
                    .swap(swap)
                    .solver(solver)
                    .apply(&mut mesh)
                    .unwrap();
                assert!(changed);
                check(
                    &mesh,
                    expected,
                    [6, 6, 0],
                    &format!("boolean {solver:?} {operation:?} swap {swap}"),
                );
            }
        }
        let mut mesh = overlapping_cubes(SelectMode::FACE, false);
        IntersectBoolean::new(Operation::Union)
            .solver(solver)
            .apply(&mut mesh)
            .unwrap();
        check(
            &mesh,
            cases[1].1,
            [0, 0, 0],
            &format!("boolean {solver:?} face mode"),
        );
    }
}

#[test]
fn operators_need_selected_faces() {
    let mut mesh = overlapping_cubes(SelectMode::EDGE, false);
    mesh.select_all(rikka_mesh_edit::SelectAction::Deselect);
    assert!(!IntersectKnife::new().apply(&mut mesh).unwrap());
    assert!(
        !IntersectBoolean::new(Operation::Union)
            .apply(&mut mesh)
            .unwrap()
    );
    assert_eq!(mesh.face_count(), 12);
}

#[test]
fn hidden_faces_survive_boolean() {
    let mut mesh = overlapping_cubes(SelectMode::EDGE, false);
    let hidden = mesh.faces().next().unwrap();
    mesh.hide_face(hidden, true);
    IntersectBoolean::new(Operation::Union)
        .apply(&mut mesh)
        .unwrap();
    mesh.validate().unwrap();
    assert!(mesh.contains_face(hidden));
    assert!(mesh.is_face_hidden(hidden));
}
