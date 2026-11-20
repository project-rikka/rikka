use rikka_mesh_modifiers::{
    Decimate, DecimateType, Delimit, Modifier, NoteKind, Purpose, SkipReason, SymmetryAxis,
    evaluate,
};
use rikka_polymesh::Polymesh;

use crate::compare::{MeshData, assert_mesh};
use crate::oracle::decimate::{
    BOX_INPUT, BUMPY_GROUP_INPUT, BUMPY_INPUT, DECIMATE_COLLAPSE, DECIMATE_FACE_COUNTS,
    DECIMATE_GROUP, DECIMATE_PLANAR, DECIMATE_PLANAR_DELIMIT, DECIMATE_SYMMETRY,
    DECIMATE_UNSUBDIVIDE, FLAT_INPUT,
};
use crate::support::{GRID, Scene, fixture};

fn decimated(mesh: &Polymesh, decimate: Decimate, scene: &Scene) -> (Polymesh, Vec<NoteKind>) {
    let stack = [Modifier::new("Decimate", decimate)];
    let evaluated = evaluate(mesh, &stack, scene, Purpose::Viewport);
    (
        evaluated.mesh,
        evaluated.notes.into_iter().map(|note| note.kind).collect(),
    )
}

#[test]
fn decimate_matches_blender() {
    let planar = Decimate {
        decimate_type: DecimateType::Planar,
        ..Decimate::default()
    };
    let cases: [(&MeshData, &[&str], Decimate, &MeshData, &str); 6] = [
        (
            &BUMPY_INPUT,
            &[],
            Decimate {
                ratio: 0.35,
                ..Decimate::default()
            },
            &DECIMATE_COLLAPSE,
            "DECIMATE_COLLAPSE",
        ),
        (
            &BUMPY_INPUT,
            &[],
            Decimate {
                ratio: 0.5,
                use_symmetry: true,
                symmetry_axis: SymmetryAxis::X,
                use_collapse_triangulate: true,
                ..Decimate::default()
            },
            &DECIMATE_SYMMETRY,
            "DECIMATE_SYMMETRY",
        ),
        (
            &BUMPY_GROUP_INPUT,
            &["Keep"],
            Decimate {
                ratio: 0.4,
                vertex_group: String::from("Keep"),
                vertex_group_factor: 3.0,
                invert_vertex_group: true,
                ..Decimate::default()
            },
            &DECIMATE_GROUP,
            "DECIMATE_GROUP",
        ),
        (
            &FLAT_INPUT,
            &[],
            Decimate {
                decimate_type: DecimateType::Unsubdivide,
                iterations: 2,
                ..Decimate::default()
            },
            &DECIMATE_UNSUBDIVIDE,
            "DECIMATE_UNSUBDIVIDE",
        ),
        (
            &BOX_INPUT,
            &[],
            planar.clone(),
            &DECIMATE_PLANAR,
            "DECIMATE_PLANAR",
        ),
        (
            &BOX_INPUT,
            &[],
            Decimate {
                angle_limit: 15.0f32.to_radians(),
                use_dissolve_boundaries: true,
                delimit: Delimit::MATERIAL | Delimit::SEAM,
                ..planar
            },
            &DECIMATE_PLANAR_DELIMIT,
            "DECIMATE_PLANAR_DELIMIT",
        ),
    ];
    let scene = Scene {
        materials: 2,
        ..Scene::default()
    };
    for (input, groups, decimate, expected, name) in cases {
        let (mesh, notes) = decimated(&input.build(groups), decimate, &scene);
        assert_mesh(&mesh, expected);
        let &(_, count) = DECIMATE_FACE_COUNTS
            .iter()
            .find(|entry| entry.0 == name)
            .expect("face count exists");
        assert_eq!(notes, [NoteKind::FaceCount(count)], "{name}");
    }
}

#[test]
fn decimate_needs_more_than_three_faces() {
    let mut mesh = fixture(&GRID);
    mesh.face_offsets.truncate(4);
    mesh.corner_verts.truncate(12);
    mesh.corner_edges.truncate(12);
    let (result, notes) = decimated(
        &mesh,
        Decimate {
            ratio: 0.5,
            ..Decimate::default()
        },
        &Scene::default(),
    );
    assert_eq!(result.face_count(), 3);
    assert_eq!(
        notes,
        [
            NoteKind::FaceCount(3),
            NoteKind::Skipped(SkipReason::TooFewFaces)
        ]
    );
}
