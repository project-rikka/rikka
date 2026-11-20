use glam::Vec3;
use rikka_mesh_modifiers::{
    Armature, Array, Bevel, Boolean, Decimate, Displace, Error, Mirror, Modifier, NoteKind,
    ObjectRef, Purpose, Shrinkwrap, SkipReason, Subdivision, apply, apply_as_shape_key, cage_index,
    evaluate,
};
use rikka_polymesh::ShapeKey;

use crate::support::{CUBE, Scene, fixture, moved};

fn separate_cube() -> rikka_polymesh::Polymesh {
    moved(fixture(&CUBE), Vec3::new(1.5, 0.0, 0.0), Vec3::ONE)
}

fn mirror() -> Mirror {
    Mirror {
        use_mirror_merge: false,
        ..Mirror::default()
    }
}

#[test]
fn hidden_modifiers_pass_mesh_through() {
    let cube = fixture(&CUBE);
    let mut hidden = Modifier::new("Subdivision", Subdivision::default());
    hidden.show_viewport = false;
    let evaluated = evaluate(&cube, &[hidden], &Scene::default(), Purpose::Viewport);
    assert_eq!(evaluated.mesh.positions, cube.positions);
    assert_eq!(
        evaluated.notes[0].kind,
        NoteKind::Skipped(SkipReason::Hidden(Purpose::Viewport))
    );
}

#[test]
fn edit_mode_respects_type_support() {
    let cube = fixture(&CUBE);
    let stack = [
        Modifier::new("Decimate", Decimate::default()),
        Modifier::new("Displace", Displace::default()),
        Modifier::new("Subdivision", Subdivision::default()),
    ];
    let evaluated = evaluate(&cube, &stack, &Scene::default(), Purpose::EditMode);
    let kinds: Vec<NoteKind> = evaluated
        .notes
        .iter()
        .map(|note| note.kind.clone())
        .collect();
    assert_eq!(
        kinds,
        vec![
            NoteKind::Skipped(SkipReason::EditModeUnsupported),
            NoteKind::Skipped(SkipReason::Hidden(Purpose::EditMode)),
        ]
    );
    assert_eq!(evaluated.mesh.face_count(), 24);
}

#[test]
fn cage_stops_at_last_on_cage_modifier() {
    let cube = separate_cube();
    let mut first = Modifier::new("Mirror", mirror());
    first.show_on_cage = true;
    let stack = [first, Modifier::new("Subdivision", Subdivision::default())];
    assert_eq!(cage_index(&stack), Some(0));
    let cage = evaluate(&cube, &stack, &Scene::default(), Purpose::EditCage);
    assert_eq!(cage.mesh.face_count(), 12);
    let edit = evaluate(&cube, &stack, &Scene::default(), Purpose::EditMode);
    assert_eq!(edit.mesh.face_count(), 48);

    let unmappable = [Modifier::new("Bevel", Bevel::default()), stack[0].clone()];
    assert_eq!(cage_index(&unmappable), None);
}

#[test]
fn origins_follow_copies() {
    let stack = [Modifier::new("Mirror", mirror())];
    let evaluated = evaluate(
        &separate_cube(),
        &stack,
        &Scene::default(),
        Purpose::Viewport,
    );
    let expected: Vec<Option<u32>> = (0..16).map(|point| Some(point % 8)).collect();
    assert_eq!(evaluated.origins.points, expected);
    let faces: Vec<Option<u32>> = (0..12).map(|face| Some(face % 6)).collect();
    assert_eq!(evaluated.origins.faces, faces);
    assert_eq!(evaluated.control_edges, None);
}

#[test]
fn subdivision_maps_children_to_cage() {
    let cube = fixture(&CUBE);
    let stack = [Modifier::new("Subdivision", Subdivision::default())];
    let evaluated = evaluate(&cube, &stack, &Scene::default(), Purpose::Viewport);
    let origins = &evaluated.origins;
    assert_eq!(origins.points.len(), 26);
    for (point, origin) in origins.points.iter().enumerate() {
        let expected = (point < 8).then_some(point as u32);
        assert_eq!(*origin, expected, "point {point}");
    }
    let control = evaluated
        .control_edges
        .expect("optimal display marks cage edges");
    for (edge, &[a, b]) in evaluated.mesh.edges.iter().enumerate() {
        match origins.edges[edge] {
            Some(parent) => {
                assert!(control[edge], "edge {edge}");
                let cage = cube.edges[parent as usize];
                let touches = |point: u32| cage.contains(&point) || point >= 8;
                assert!(
                    touches(a) && touches(b),
                    "edge {edge} lies on cage edge {parent}"
                );
            }
            None => assert!(!control[edge], "edge {edge}"),
        }
    }
    let faces: Vec<Option<u32>> = (0..24).map(|face| Some(face / 4)).collect();
    assert_eq!(origins.faces, faces);
}

#[test]
fn stacking_order_matters() {
    let cube = separate_cube();
    let array = Array {
        count: 2,
        ..Array::default()
    };
    let mirror_first = [
        Modifier::new("Mirror", mirror()),
        Modifier::new("Array", array.clone()),
    ];
    let array_first = [
        Modifier::new("Array", array),
        Modifier::new("Mirror", mirror()),
    ];
    let scene = Scene::default();
    let a = evaluate(&cube, &mirror_first, &scene, Purpose::Viewport).mesh;
    let b = evaluate(&cube, &array_first, &scene, Purpose::Viewport).mesh;
    assert_eq!(a.point_count(), b.point_count());
    let extent = |mesh: &rikka_polymesh::Polymesh| {
        mesh.positions
            .iter()
            .fold(f32::MIN, |high, position| high.max(position.x))
    };
    assert!((extent(&a) - 7.5).abs() < 1.0e-5);
    assert!((extent(&b) - 4.5).abs() < 1.0e-5);
}

#[test]
fn shape_keys_travel_through_modifiers() {
    let mut cube = separate_cube();
    let lifted = cube
        .positions
        .iter()
        .map(|position| *position + Vec3::Z)
        .collect();
    cube.shape_keys
        .add(ShapeKey::new("Basis", cube.positions.clone()));
    cube.shape_keys.add(ShapeKey::new("Lift", lifted));
    let stack = [
        Modifier::new("Mirror", mirror()),
        Modifier::new("Subdivision", Subdivision::default()),
    ];
    let evaluated = evaluate(&cube, &stack, &Scene::default(), Purpose::Viewport).mesh;
    let key = &evaluated.shape_keys.keys[1];
    for (position, keyed) in evaluated.positions.iter().zip(&key.positions) {
        assert!((*keyed - *position - Vec3::Z).length() < 1.0e-5);
    }
}

#[test]
fn scene_kinds_and_missing_references_are_skipped() {
    let cube = fixture(&CUBE);
    let stack = [
        Modifier::new("Armature", Armature::default()),
        Modifier::new(
            "Armature",
            Armature {
                object: Some(ObjectRef::new(0)),
                ..Armature::default()
            },
        ),
        Modifier::new("Boolean", Boolean::default()),
        Modifier::new("Shrinkwrap", Shrinkwrap::default()),
    ];
    let evaluated = evaluate(&cube, &stack, &Scene::default(), Purpose::Viewport);
    let kinds: Vec<NoteKind> = evaluated
        .notes
        .iter()
        .map(|note| note.kind.clone())
        .collect();
    assert_eq!(
        kinds,
        vec![
            NoteKind::Skipped(SkipReason::MissingObject),
            NoteKind::Skipped(SkipReason::SceneEvaluated),
            NoteKind::Skipped(SkipReason::MissingObject),
            NoteKind::Skipped(SkipReason::MissingObject),
        ]
    );
    assert_eq!(evaluated.mesh.positions, cube.positions);
}

#[test]
fn apply_follows_blender_rules() {
    let scene = Scene::default();
    let mut cube = separate_cube();
    apply(&mut cube, &Modifier::new("Mirror", mirror()), &scene).expect("mirror applies");
    assert_eq!(cube.point_count(), 16);

    let mut keyed = separate_cube();
    keyed
        .shape_keys
        .add(ShapeKey::new("Basis", keyed.positions.clone()));
    let refused = apply(&mut keyed, &Modifier::new("Mirror", mirror()), &scene);
    assert!(matches!(refused, Err(Error::ShapeKeys { .. })));

    let displace = Modifier::new("Displace", Displace::default());
    let mut plain = separate_cube();
    let key = apply_as_shape_key(&mut plain, &displace, &scene).expect("displace deforms");
    assert_eq!(key, 1);
    assert_eq!(plain.shape_keys.keys[0].positions, plain.positions);
    assert_ne!(plain.shape_keys.keys[1].positions, plain.positions);

    let not_deforming = apply_as_shape_key(&mut plain, &Modifier::new("Mirror", mirror()), &scene);
    assert!(matches!(not_deforming, Err(Error::NotDeforming { .. })));
    let disabled = apply(
        &mut plain,
        &Modifier::new("Shrinkwrap", Shrinkwrap::default()),
        &scene,
    );
    assert!(matches!(disabled, Err(Error::Disabled { .. })));
}
