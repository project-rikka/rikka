use glam::{Mat4, Vec2, Vec3};
use rikka_polymesh::{Domain, EdgeKind, Influence, MergeData, Polymesh, ShapeKey, Values};

use crate::support::{cube, points};

fn face_sets(mesh: &Polymesh) -> Vec<Vec<u32>> {
    let mut faces: Vec<Vec<u32>> = (0..mesh.face_count())
        .map(|face| {
            let mut points = mesh.face_verts(face).to_vec();
            points.sort_unstable();
            points
        })
        .collect();
    faces.sort();
    faces
}

#[test]
fn mirrored_transform_keeps_faces_outward() {
    let mut mesh = cube();
    mesh.custom_normals_mut().fill(Vec3::X);
    mesh.transform(&Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0)));
    mesh.validate().expect("transform keeps the mesh valid");
    for face in 0..mesh.face_count() {
        let center = mesh
            .face_verts(face)
            .iter()
            .map(|&point| mesh.positions[point as usize])
            .sum::<Vec3>()
            / 4.0;
        assert!(
            mesh.face_normal(face).dot(center) > 0.0,
            "face {face} points inward"
        );
    }
    assert!(
        mesh.custom_normals()
            .unwrap()
            .iter()
            .all(|normal| *normal == Vec3::NEG_X)
    );
}

#[test]
fn flipping_reverses_corner_data_and_keeps_first_corner() {
    let mut mesh = cube();
    mesh.add_uv_map(
        "UVMap",
        (0..24).map(|corner| Vec2::splat(corner as f32)).collect(),
    );
    mesh.flip_faces();
    mesh.validate().expect("flipped meshes are valid");
    assert_eq!(mesh.face_verts(0), &[0, 1, 2, 3]);
    assert_eq!(
        &mesh.uv_map("UVMap").unwrap()[..4],
        &[
            Vec2::ZERO,
            Vec2::splat(3.0),
            Vec2::splat(2.0),
            Vec2::splat(1.0)
        ]
    );
}

#[test]
fn join_unions_layers_groups_and_keys_by_name() {
    let mut first = cube();
    first.vertex_groups.add("Spine");
    first.vertex_groups.ensure_weights(8);
    first.vertex_groups.set_weight(0, 0, 1.0);
    first
        .shape_keys
        .add(ShapeKey::new("Basis", first.positions.clone()));
    let mut second = cube();
    second.add_uv_map("UVMap", vec![Vec2::ONE; 24]);
    second.vertex_groups.add("Arm");
    second.vertex_groups.add("Spine");
    second.vertex_groups.ensure_weights(8);
    second.vertex_groups.set_weight(1, 1, 0.5);
    let mut lifted = second.positions.clone();
    lifted.iter_mut().for_each(|position| position.z += 1.0);
    second
        .shape_keys
        .add(ShapeKey::new("Basis", second.positions.clone()));
    second.shape_keys.add(ShapeKey::new("Lift", lifted));

    let joined = Polymesh::join(&[
        (&first, Mat4::IDENTITY),
        (&second, Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0))),
    ]);
    joined.validate().expect("joined meshes are valid");
    assert_eq!(
        (
            joined.point_count(),
            joined.edge_count(),
            joined.face_count()
        ),
        (16, 24, 12)
    );
    let uvs = joined.uv_map("UVMap").expect("UV maps are united");
    assert_eq!((uvs[0], uvs[24]), (Vec2::ZERO, Vec2::ONE));
    let names: Vec<&str> = joined
        .vertex_groups
        .groups
        .iter()
        .map(|group| group.name.as_str())
        .collect();
    assert_eq!(names, ["Spine", "Arm"]);
    assert_eq!(
        joined.vertex_groups.weights[9],
        vec![Influence {
            group: 0,
            weight: 0.5
        }]
    );
    let lift = joined.shape_keys.get("Lift").expect("keys are united");
    assert_eq!(lift.positions[0], first.positions[0]);
    assert_eq!(lift.positions[8], Vec3::new(4.0, -1.0, 0.0));
}

#[test]
fn merging_welds_shared_edges() {
    let mut mesh = Polymesh::from_polygons(
        points(&[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ]),
        [[0u32, 1, 2, 3], [4, 5, 6, 7]],
    );
    mesh.attributes.insert(rikka_polymesh::Attribute::new(
        "heat",
        Domain::Point,
        Values::Float(vec![0.0, 1.0, 1.0, 0.0, 3.0, 0.0, 0.0, 3.0]),
    ));
    mesh.merge_points(&[0, 1, 2, 3, 1, 5, 6, 2], MergeData::Mix);
    mesh.validate().expect("merged meshes are valid");
    assert_eq!(
        (mesh.point_count(), mesh.edge_count(), mesh.face_count()),
        (6, 7, 2)
    );
    assert_eq!(
        mesh.attributes
            .values::<f32>("heat", Domain::Point)
            .unwrap(),
        &[0.0, 2.0, 2.0, 0.0, 0.0, 0.0]
    );
    assert_eq!(
        mesh.edge_kinds()
            .iter()
            .filter(|kind| **kind == EdgeKind::Manifold)
            .count(),
        1
    );
}

#[test]
fn merging_collapses_degenerate_and_duplicate_faces() {
    let quad = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let mut collapsed = Polymesh::from_polygons(points(&quad), [[0u32, 1, 2, 3]]);
    collapsed.merge_points(&[0, 0, 2, 3], MergeData::Mix);
    assert_eq!(face_sets(&collapsed), vec![vec![0, 1, 2]]);
    assert_eq!(collapsed.edge_count(), 3);
    assert_eq!(collapsed.positions[0], Vec3::new(0.5, 0.0, 0.0));

    let doubled_points: Vec<[f32; 3]> = quad.iter().chain(&quad).copied().collect();
    let mut doubled =
        Polymesh::from_polygons(points(&doubled_points), [[0u32, 1, 2, 3], [4, 5, 6, 7]]);
    doubled.merge_points(&[0, 1, 2, 3, 0, 1, 2, 3], MergeData::KeepTarget);
    doubled.validate().expect("merged meshes are valid");
    assert_eq!(
        (
            doubled.point_count(),
            doubled.edge_count(),
            doubled.face_count()
        ),
        (4, 4, 1)
    );
}

#[test]
fn merging_splits_faces_that_touch_themselves() {
    let ring: Vec<[f32; 3]> = (0..6)
        .map(|point| {
            let angle = std::f32::consts::TAU * point as f32 / 6.0;
            [angle.cos(), angle.sin(), 0.0]
        })
        .collect();
    let mut mesh = Polymesh::from_polygons(points(&ring), [[0u32, 1, 2, 3, 4, 5]]);
    mesh.merge_points(&[0, 1, 2, 0, 4, 5], MergeData::KeepTarget);
    mesh.validate().expect("split faces are valid");
    assert_eq!(face_sets(&mesh), vec![vec![0, 1, 2], vec![0, 3, 4]]);
}

#[test]
fn loose_geometry_is_removed_and_faces_extracted() {
    let mut mesh = cube();
    mesh.positions.push(Vec3::splat(9.0));
    mesh.resize(Domain::Point, 10);
    mesh.edges.push([8, 9]);
    mesh.resize(Domain::Edge, 13);
    mesh.edges[12] = [8, 9];
    assert!(mesh.remove_loose());
    assert_eq!((mesh.point_count(), mesh.edge_count()), (8, 12));
    assert!(!mesh.remove_loose());

    mesh.material_indices_mut()[1] = 7;
    let part = mesh.extract(&[1]);
    part.validate().expect("extracted parts are valid");
    assert_eq!(
        (part.point_count(), part.edge_count(), part.face_count()),
        (4, 4, 1)
    );
    assert_eq!(part.material_index(0), 7);
}

#[test]
fn deleting_points_removes_their_edges_and_faces() {
    let mut mesh = cube();
    let mut keep = vec![true; 8];
    keep[0] = false;
    mesh.retain_points(&keep);
    mesh.validate().expect("deletion keeps the mesh valid");
    assert_eq!(
        (mesh.point_count(), mesh.edge_count(), mesh.face_count()),
        (7, 9, 3)
    );
}

#[test]
fn components_and_uv_islands_follow_connectivity() {
    let mut mesh = Polymesh::join(&[
        (&cube(), Mat4::IDENTITY),
        (&cube(), Mat4::from_translation(Vec3::X * 4.0)),
    ]);
    assert_eq!(mesh.point_components().count, 2);
    assert_eq!(mesh.face_components().count, 2);
    let uvs: Vec<Vec2> = mesh
        .corner_verts
        .iter()
        .map(|&point| mesh.positions[point as usize].truncate())
        .collect();
    mesh.add_uv_map("UVMap", uvs);
    assert_eq!(
        mesh.uv_islands("UVMap", false).map(|islands| islands.count),
        Some(2)
    );
    mesh.uv_seams_mut().fill(true);
    assert_eq!(
        mesh.uv_islands("UVMap", true).map(|islands| islands.count),
        Some(12)
    );
    assert!(mesh.is_manifold());
}
