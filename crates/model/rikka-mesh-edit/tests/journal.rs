use std::collections::BTreeSet;

use glam::Vec3;
use rikka_mesh_edit::{
    EditMesh, Element, ElementId, Flags, LinkedDelimit, SelectAction, SelectMode,
};

use crate::support::{grid, vert};

type State = (Vec<Vec3>, Vec<Flags>, Vec<Flags>, Vec<Flags>);
type Edit = (&'static str, Box<dyn Fn(&mut EditMesh)>);

fn state(mesh: &EditMesh) -> State {
    (
        mesh.verts().map(|vert| mesh.position(vert)).collect(),
        mesh.verts().map(|vert| mesh.vert_flags(vert)).collect(),
        mesh.edges().map(|edge| mesh.edge_flags(edge)).collect(),
        mesh.faces().map(|face| mesh.face_flags(face)).collect(),
    )
}

fn differing<T: PartialEq>(
    before: &[T],
    after: &[T],
    same: impl Fn(&T, &T) -> bool,
) -> BTreeSet<usize> {
    before
        .iter()
        .zip(after)
        .enumerate()
        .filter(|(_, (a, b))| !same(a, b))
        .map(|(index, _)| index)
        .collect()
}

fn watched(a: &Flags, b: &Flags) -> bool {
    (*a ^ *b) & (Flags::SELECT | Flags::HIDDEN) == Flags::empty()
}

fn indices<I: ElementId>(ids: &[I]) -> BTreeSet<usize> {
    ids.iter().map(|id| id.index()).collect()
}

#[test]
fn journal_covers_every_selection_and_position_change() {
    let edits: Vec<Edit> = vec![
        (
            "select vert",
            Box::new(|mesh| mesh.select_vert(vert(7), true)),
        ),
        ("flush", Box::new(|mesh| mesh.select_mode_flush())),
        (
            "select all",
            Box::new(|mesh| mesh.select_all(SelectAction::Select)),
        ),
        (
            "deselect all",
            Box::new(|mesh| mesh.select_all(SelectAction::Deselect)),
        ),
        (
            "linked pick",
            Box::new(|mesh| {
                mesh.select_linked_pick(Element::Vert(vert(3)), true, LinkedDelimit::empty())
            }),
        ),
        ("less", Box::new(|mesh| mesh.select_less(true))),
        ("more", Box::new(|mesh| mesh.select_more(true))),
        (
            "hide",
            Box::new(|mesh| {
                mesh.hide_selected(false);
            }),
        ),
        (
            "reveal",
            Box::new(|mesh| {
                mesh.reveal(true);
            }),
        ),
        (
            "face mode",
            Box::new(|mesh| mesh.change_select_mode(SelectMode::FACE, true)),
        ),
        (
            "invert",
            Box::new(|mesh| mesh.select_all(SelectAction::Invert)),
        ),
        (
            "move",
            Box::new(|mesh| {
                mesh.set_position(vert(4), Vec3::new(9.0, 9.0, 9.0));
                mesh.translate([vert(5), vert(6)], Vec3::X);
            }),
        ),
    ];
    let mut mesh = grid(6, 5);
    for (name, edit) in &edits {
        let before = state(&mesh);
        let mark = mesh.journal_mark();
        edit(&mut mesh);
        let after = state(&mesh);
        let changes = mesh
            .changes_since(mark)
            .unwrap_or_else(|| panic!("{name}: the journal stays valid"));
        let moved = differing(&before.0, &after.0, |a, b| a == b);
        assert!(moved.is_subset(&indices(changes.moved)), "{name}: moved");
        assert!(
            differing(&before.1, &after.1, watched).is_subset(&indices(changes.verts)),
            "{name}: verts"
        );
        assert!(
            differing(&before.2, &after.2, watched).is_subset(&indices(changes.edges)),
            "{name}: edges"
        );
        assert!(
            differing(&before.3, &after.3, watched).is_subset(&indices(changes.faces)),
            "{name}: faces"
        );
    }
}

#[test]
fn journal_marks_expire_on_topology_change_and_clone() {
    let mut mesh = grid(3, 3);
    let mark = mesh.journal_mark();
    mesh.select_vert(vert(1), true);
    assert!(mesh.changes_since(mark).is_some());
    let copy = mesh.clone();
    assert!(copy.changes_since(mark).is_none());
    let faces: Vec<_> = mesh.faces().take(1).collect();
    mesh.kill_face(faces[0]).expect("the face exists");
    assert!(mesh.changes_since(mark).is_none());
    let mark = mesh.journal_mark();
    mesh.positions_mut()[0] = Vec3::ONE;
    assert!(mesh.changes_since(mark).is_none());
}
