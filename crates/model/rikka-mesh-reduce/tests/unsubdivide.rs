use rikka_mesh_edit::SelectMode;
use rikka_mesh_reduce::{Error, UnsubdivideOptions, decimate_unsubdivide, unsubdivide};

use crate::oracle::{decimate as modifier, reduce as oracle};
use crate::support::{
    Mesh, Outcome, Pick, all_verts, assert_mesh, assert_outcome, edit_mesh, select,
};

fn run(fixture: &Mesh, verts: Option<&'static [u32]>, iterations: u32, expected: &Outcome) {
    let mut mesh = edit_mesh(fixture, SelectMode::VERT);
    let verts = verts.unwrap_or_else(|| Vec::leak(all_verts(&mesh)));
    select(
        &mut mesh,
        Pick {
            verts,
            ..Pick::default()
        },
    );
    unsubdivide(&mut mesh, &UnsubdivideOptions { iterations }).expect("valid iterations");
    assert_outcome(&mesh, expected);
}

#[test]
fn unsubdivide_matches_blender() {
    run(&oracle::UNSUB_FIXTURE, None, 1, &oracle::UNSUBDIVIDE);
    run(&oracle::UNSUB_FIXTURE, None, 2, &oracle::UNSUBDIVIDE_TWICE);
    run(
        &oracle::UNSUB_FIXTURE,
        Some(&[0, 1, 2, 5, 6, 7, 10, 11, 12]),
        2,
        &oracle::UNSUBDIVIDE_PART,
    );
    run(&oracle::CUBE_SUB, None, 2, &oracle::UNSUBDIVIDE_CUBE);
}

#[test]
fn unsubdivide_rejects_zero_iterations() {
    let mut mesh = edit_mesh(&oracle::UNSUB_FIXTURE, SelectMode::VERT);
    let result = unsubdivide(&mut mesh, &UnsubdivideOptions { iterations: 0 });
    assert!(matches!(result, Err(Error::OutOfRange("iterations"))));
}

#[test]
fn unsubdivide_modifier_matches_blender() {
    let mut mesh = edit_mesh(&modifier::SUBDIVIDED, SelectMode::VERT);
    decimate_unsubdivide(&mut mesh, 2);
    assert_mesh(&mesh, &modifier::MODIFIER_UNSUBDIVIDE);
}
