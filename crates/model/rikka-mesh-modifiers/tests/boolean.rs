use glam::Mat4;
use rikka_mesh_modifiers::{
    Boolean, BooleanMaterialMode, BooleanOperand, BooleanOperation, BooleanSolver, CollectionRef,
    MaterialRef, Modifier, NoteKind, ObjectRef, Purpose, SkipReason, evaluate,
};
use rikka_polymesh::Polymesh;

use crate::compare::{MeshData, assert_mesh};
use crate::oracle::boolean::{
    BOOLEAN_COLLECTION_FAST, BOOLEAN_COLLECTION_UNION, BOOLEAN_CUBE, BOOLEAN_DIFFERENCE_EXACT,
    BOOLEAN_DIFFERENCE_FAST, BOOLEAN_INTERSECT_EXACT, BOOLEAN_MATERIAL_INDEX,
    BOOLEAN_MATERIAL_TRANSFER, BOOLEAN_SUMMARY, BOOLEAN_TRANSFER_SLOTS, BOOLEAN_UNION_FAST,
    CUTTER_MATRIX, OTHER_MATRIX, PLAIN_MATRIX, TARGET_MATRIX,
};
use crate::support::{Scene, run_in};

const CUTTER: ObjectRef = ObjectRef::new(0);
const CUTTERS: CollectionRef = CollectionRef::new(0);
const PLAIN_CUTTERS: CollectionRef = CollectionRef::new(1);

fn cube_without_uvs() -> Polymesh {
    let mut mesh = BOOLEAN_CUBE.build(&[]);
    mesh.attributes.remove("UVMap");
    mesh.uv_maps = Default::default();
    mesh
}

fn scene() -> Scene {
    Scene {
        matrix: Mat4::from_cols_array(&TARGET_MATRIX),
        ..Scene::default()
    }
    .with_object(
        Mat4::from_cols_array(&CUTTER_MATRIX),
        Some(BOOLEAN_CUBE.build(&[])),
    )
    .with_object(
        Mat4::from_cols_array(&OTHER_MATRIX),
        Some(cube_without_uvs()),
    )
    .with_object(
        Mat4::from_cols_array(&PLAIN_MATRIX),
        Some(cube_without_uvs()),
    )
}

fn collection_scene() -> Scene {
    Scene {
        collections: vec![
            vec![ObjectRef::new(0), ObjectRef::new(1)],
            vec![ObjectRef::new(0), ObjectRef::new(2)],
        ],
        ..scene()
    }
}

fn material(index: u64) -> Option<MaterialRef> {
    Some(MaterialRef::new(index))
}

fn material_scene() -> Scene {
    let mut scene = scene();
    scene.materials = 2;
    scene.slots = Some(vec![material(0), material(1)]);
    scene.object_slots = vec![Some(vec![material(1), material(2)]), None];
    if let Some((_, Some(cutter))) = scene.objects.first_mut() {
        let mut mesh = (**cutter).clone();
        for face in 0..mesh.face_count() {
            mesh.material_indices_mut()[face] = ((face + 1) % 2) as i32;
        }
        *cutter = std::sync::Arc::new(mesh);
    }
    scene
}

fn material_target() -> Polymesh {
    let mut mesh = BOOLEAN_CUBE.build(&[]);
    for face in 0..mesh.face_count() {
        mesh.material_indices_mut()[face] = (face % 2) as i32;
    }
    mesh
}

fn object(operation: BooleanOperation, solver: BooleanSolver) -> Boolean {
    Boolean {
        operation,
        solver,
        object: Some(CUTTER),
        ..Boolean::default()
    }
}

fn summary(name: &str) -> (usize, usize, usize, f32) {
    let &(_, points, edges, faces, volume) = BOOLEAN_SUMMARY
        .iter()
        .find(|entry| entry.0 == name)
        .expect("summary exists");
    (points, edges, faces, volume)
}

fn assert_summary(mesh: &Polymesh, name: &str) {
    let (points, edges, faces, volume) = summary(name);
    assert_eq!(
        (mesh.point_count(), mesh.edge_count(), mesh.face_count()),
        (points, edges, faces),
        "{name} counts"
    );
    let measured = mesh_volume(mesh);
    assert!(
        (measured - volume).abs() < 1.0e-4,
        "{name} volume {measured} != {volume}"
    );
}

fn mesh_volume(mesh: &Polymesh) -> f32 {
    let mut total = 0.0;
    for face in 0..mesh.face_count() {
        let points = mesh.face_verts(face);
        let first = mesh.positions[points[0] as usize];
        for pair in points[1..].windows(2) {
            let b = mesh.positions[pair[0] as usize];
            let c = mesh.positions[pair[1] as usize];
            total += first.dot(b.cross(c)) / 6.0;
        }
    }
    total
}

#[test]
fn boolean_matches_blender() {
    let cases: [(Boolean, &str, &MeshData, bool); 6] = [
        (
            object(BooleanOperation::Difference, BooleanSolver::Exact),
            "BOOLEAN_DIFFERENCE_EXACT",
            &BOOLEAN_DIFFERENCE_EXACT,
            false,
        ),
        (
            object(BooleanOperation::Union, BooleanSolver::Fast),
            "BOOLEAN_UNION_FAST",
            &BOOLEAN_UNION_FAST,
            false,
        ),
        (
            object(BooleanOperation::Difference, BooleanSolver::Fast),
            "BOOLEAN_DIFFERENCE_FAST",
            &BOOLEAN_DIFFERENCE_FAST,
            false,
        ),
        (
            Boolean {
                use_self: true,
                use_hole_tolerant: true,
                ..object(BooleanOperation::Intersect, BooleanSolver::Exact)
            },
            "BOOLEAN_INTERSECT_EXACT",
            &BOOLEAN_INTERSECT_EXACT,
            false,
        ),
        (
            Boolean {
                operand_type: BooleanOperand::Collection,
                collection: Some(CUTTERS),
                ..object(BooleanOperation::Union, BooleanSolver::Exact)
            },
            "BOOLEAN_COLLECTION_UNION",
            &BOOLEAN_COLLECTION_UNION,
            true,
        ),
        (
            Boolean {
                operand_type: BooleanOperand::Collection,
                collection: Some(PLAIN_CUTTERS),
                ..object(BooleanOperation::Difference, BooleanSolver::Fast)
            },
            "BOOLEAN_COLLECTION_FAST",
            &BOOLEAN_COLLECTION_FAST,
            true,
        ),
    ];
    for (boolean, name, expected, collection) in cases {
        let scene = match collection {
            true => collection_scene(),
            false => scene(),
        };
        let evaluated = run_in(&BOOLEAN_CUBE.build(&[]), boolean, &scene);
        assert_summary(&evaluated.mesh, name);
        assert_mesh(&evaluated.mesh, expected);
    }
}

#[test]
fn boolean_material_index_remaps_slots() {
    let boolean = object(BooleanOperation::Difference, BooleanSolver::Exact);
    let evaluated = run_in(&material_target(), boolean, &material_scene());
    assert_mesh(&evaluated.mesh, &BOOLEAN_MATERIAL_INDEX);
}

#[test]
fn boolean_material_transfer_grows_slots() {
    let boolean = Boolean {
        material_mode: BooleanMaterialMode::Transfer,
        ..object(BooleanOperation::Difference, BooleanSolver::Exact)
    };
    let stack = [Modifier::new("Boolean", boolean)];
    let evaluated = evaluate(
        &material_target(),
        &stack,
        &material_scene(),
        Purpose::Viewport,
    );
    assert_mesh(&evaluated.mesh, &BOOLEAN_MATERIAL_TRANSFER);
    let names = ["A", "B", "C"];
    let expected: Vec<Option<MaterialRef>> = BOOLEAN_TRANSFER_SLOTS
        .iter()
        .map(|name| {
            let index = names
                .iter()
                .position(|known| known == name)
                .expect("known material");
            material(index as u64)
        })
        .collect();
    assert_eq!(
        evaluated
            .notes
            .iter()
            .map(|note| &note.kind)
            .collect::<Vec<_>>(),
        [&NoteKind::MaterialSlots(expected)]
    );
}

#[test]
fn boolean_keeps_target_face_origins() {
    let boolean = object(BooleanOperation::Difference, BooleanSolver::Exact);
    let mesh = BOOLEAN_CUBE.build(&[]);
    let evaluated = run_in(&mesh, boolean, &scene());
    let input_normals = mesh.face_normals();
    let normals = evaluated.mesh.face_normals();
    for (face, origin) in evaluated.origins.faces.iter().enumerate() {
        if let Some(origin) = origin {
            assert!(
                normals[face].abs_diff_eq(input_normals[*origin as usize], 1.0e-5),
                "face {face} maps to {origin}"
            );
        }
    }
    let kept = evaluated.origins.faces.iter().flatten().count();
    let cut = evaluated
        .origins
        .faces
        .iter()
        .filter(|origin| origin.is_none())
        .count();
    assert_eq!(kept + cut, evaluated.mesh.face_count());
    assert!(kept >= 6 && cut >= 3, "{kept} kept, {cut} cut");
}

#[test]
fn boolean_reports_blender_errors() {
    let cases: [(Boolean, Scene, SkipReason); 4] = [
        (
            Boolean {
                operand_type: BooleanOperand::Collection,
                collection: Some(CUTTERS),
                ..object(BooleanOperation::Intersect, BooleanSolver::Fast)
            },
            collection_scene(),
            SkipReason::IntersectNeedsExact,
        ),
        (
            Boolean {
                operand_type: BooleanOperand::Collection,
                collection: Some(CUTTERS),
                ..object(BooleanOperation::Union, BooleanSolver::Fast)
            },
            Scene {
                collections: vec![Vec::new()],
                ..scene()
            },
            SkipReason::EmptyCollection,
        ),
        (
            Boolean {
                operand_type: BooleanOperand::Collection,
                collection: Some(CUTTERS),
                ..object(BooleanOperation::Union, BooleanSolver::Exact)
            },
            Scene {
                collections: vec![vec![ObjectRef::new(0), ObjectRef::new(7)]],
                ..scene()
            },
            SkipReason::NonMeshInCollection,
        ),
        (
            Boolean {
                operand_type: BooleanOperand::Collection,
                ..object(BooleanOperation::Union, BooleanSolver::Fast)
            },
            scene(),
            SkipReason::MissingObject,
        ),
    ];
    let mesh = BOOLEAN_CUBE.build(&[]);
    for (boolean, scene, reason) in cases {
        let stack = [Modifier::new("Boolean", boolean)];
        let evaluated = evaluate(&mesh, &stack, &scene, Purpose::Viewport);
        assert_eq!(evaluated.mesh.positions, mesh.positions);
        assert_eq!(
            evaluated
                .notes
                .iter()
                .map(|note| &note.kind)
                .collect::<Vec<_>>(),
            [&NoteKind::Skipped(reason)]
        );
    }
}

#[test]
fn manifold_solver_runs_exact() {
    let boolean = object(BooleanOperation::Difference, BooleanSolver::Manifold);
    let stack = [Modifier::new("Boolean", boolean)];
    let evaluated = evaluate(
        &BOOLEAN_CUBE.build(&[]),
        &stack,
        &scene(),
        Purpose::Viewport,
    );
    assert_summary(&evaluated.mesh, "BOOLEAN_DIFFERENCE_EXACT");
    assert_eq!(
        evaluated
            .notes
            .iter()
            .map(|note| &note.kind)
            .collect::<Vec<_>>(),
        [&NoteKind::ManifoldSolverAsExact]
    );
}
