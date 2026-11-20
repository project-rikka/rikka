use rikka_mesh_edit::{EditMesh, SelectMode};
use rikka_mesh_reduce::{
    Axis, Collapse, DecimateOptions, Delimit, Error, decimate, decimate_collapse, decimate_planar,
};

use crate::oracle::decimate as oracle;
use crate::support::{
    Mesh, Outcome, Pick, all_verts, assert_mesh, assert_outcome, edit_mesh, select,
};

fn selected(fixture: &Mesh, verts: Option<&'static [u32]>) -> EditMesh {
    let mut mesh = edit_mesh(fixture, SelectMode::VERT);
    let verts = verts.unwrap_or_else(|| Vec::leak(all_verts(&mesh)));
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
fn decimate_sphere_matches_blender() {
    let cases: [(DecimateOptions, &Outcome); 5] = [
        (
            DecimateOptions {
                ratio: 0.5,
                ..DecimateOptions::default()
            },
            &oracle::SPHERE_HALF,
        ),
        (
            DecimateOptions {
                ratio: 0.25,
                ..DecimateOptions::default()
            },
            &oracle::SPHERE_QUARTER,
        ),
        (
            DecimateOptions {
                ratio: 0.4,
                use_vertex_group: true,
                ..DecimateOptions::default()
            },
            &oracle::SPHERE_GROUP,
        ),
        (
            DecimateOptions {
                ratio: 0.4,
                use_vertex_group: true,
                invert_vertex_group: true,
                vertex_group_factor: 3.0,
                ..DecimateOptions::default()
            },
            &oracle::SPHERE_INVERT,
        ),
        (
            DecimateOptions {
                ratio: 0.3,
                use_symmetry: true,
                symmetry_axis: Axis::Z,
                ..DecimateOptions::default()
            },
            &oracle::SPHERE_SYMMETRY,
        ),
    ];
    for (options, expected) in cases {
        let mut mesh = selected(&oracle::SPHERE, None);
        decimate(&mut mesh, &options).expect("valid options");
        assert_outcome(&mesh, expected);
    }
}

#[test]
fn decimate_selection_matches_blender() {
    let verts: Vec<u32> = (0..114).step_by(2).collect();
    let mut mesh = selected(&oracle::SPHERE, Some(Vec::leak(verts)));
    decimate(
        &mut mesh,
        &DecimateOptions {
            ratio: 0.5,
            ..DecimateOptions::default()
        },
    )
    .expect("valid options");
    assert_outcome(&mesh, &oracle::SPHERE_PART);
}

#[test]
fn decimate_monkey_matches_blender() {
    let cases: [(DecimateOptions, &Outcome); 3] = [
        (
            DecimateOptions {
                ratio: 0.5,
                ..DecimateOptions::default()
            },
            &oracle::MONKEY_HALF,
        ),
        (
            DecimateOptions {
                ratio: 0.2,
                ..DecimateOptions::default()
            },
            &oracle::MONKEY_FIFTH,
        ),
        (
            DecimateOptions {
                ratio: 0.3,
                use_symmetry: true,
                symmetry_axis: Axis::X,
                ..DecimateOptions::default()
            },
            &oracle::MONKEY_SYMMETRY,
        ),
    ];
    for (options, expected) in cases {
        let mut mesh = selected(&oracle::MONKEY, None);
        decimate(&mut mesh, &options).expect("valid options");
        assert_outcome(&mesh, expected);
    }
}

#[test]
fn decimate_rejects_ratio_above_one() {
    let mut mesh = selected(&oracle::SPHERE, None);
    let result = decimate(
        &mut mesh,
        &DecimateOptions {
            ratio: 1.5,
            ..DecimateOptions::default()
        },
    );
    assert!(matches!(result, Err(Error::OutOfRange("ratio"))));
}

fn sphere_weights(mesh: &EditMesh) -> Vec<f32> {
    let mut weights = vec![0.0; mesh.vert_capacity()];
    for vert in mesh.verts() {
        weights[vert.index()] = mesh.vert_weight(vert, 0);
    }
    weights
}

#[test]
fn decimate_modifier_collapse_matches_blender() {
    let mesh = edit_mesh(&oracle::SPHERE, SelectMode::VERT);
    let weights = sphere_weights(&mesh);
    let cases: [(&Mesh, Collapse, &Mesh); 4] = [
        (
            &oracle::SPHERE,
            Collapse {
                factor: 0.4,
                ..Collapse::default()
            },
            &oracle::MODIFIER_COLLAPSE,
        ),
        (
            &oracle::SPHERE,
            Collapse {
                factor: 0.4,
                triangulate: true,
                ..Collapse::default()
            },
            &oracle::MODIFIER_TRIANGULATE,
        ),
        (
            &oracle::SPHERE,
            Collapse {
                factor: 0.4,
                weights: Some(weights),
                weight_factor: 2.0,
                ..Collapse::default()
            },
            &oracle::MODIFIER_GROUP,
        ),
        (
            &oracle::MONKEY,
            Collapse {
                factor: 0.35,
                symmetry: Some(Axis::X),
                ..Collapse::default()
            },
            &oracle::MODIFIER_MONKEY,
        ),
    ];
    for (fixture, collapse, expected) in cases {
        let mut mesh = edit_mesh(fixture, SelectMode::VERT);
        decimate_collapse(&mut mesh, &collapse);
        assert_mesh(&mesh, expected);
    }
}

#[test]
fn decimate_modifier_planar_matches_blender() {
    let mut mesh = edit_mesh(&oracle::MONKEY, SelectMode::VERT);
    decimate_planar(&mut mesh, 15.0f32.to_radians(), false, Delimit::empty());
    assert_mesh(&mesh, &oracle::MODIFIER_PLANAR);

    let mut mesh = edit_mesh(&oracle::MONKEY, SelectMode::VERT);
    decimate_planar(&mut mesh, 25.0f32.to_radians(), true, Delimit::UV);
    assert_mesh(&mesh, &oracle::MODIFIER_PLANAR_BOUNDARY);
}
