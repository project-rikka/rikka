use std::time::Instant;

use glam::{Vec2, Vec3, Vec4};
use rikka_mesh_edit::{EditMesh, VertId};
use rikka_polymesh::{Attribute, Domain, Polymesh, ShapeKey, Values, VertexGroup};

use crate::support::{TOLERANCE, checked};

fn rich_mesh() -> Polymesh {
    let positions = vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(2.0, 0.0, 0.0),
        Vec3::new(2.0, 1.0, 0.0),
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(1.5, 2.0, 0.0),
        Vec3::new(5.0, 5.0, 5.0),
        Vec3::new(6.0, 5.0, 5.0),
        Vec3::new(7.0, 7.0, 7.0),
    ];
    let mut mesh = Polymesh::from_polygons(positions, [vec![0, 1, 4, 5], vec![1, 2, 3, 6, 4]]);
    mesh.edges.push([7, 8]);
    let corners = mesh.corner_count();
    mesh.add_uv_map(
        "UVMap",
        (0..corners)
            .map(|corner| Vec2::new(corner as f32 * 0.1, 0.5))
            .collect(),
    );
    mesh.uv_pins_mut("UVMap")[2] = true;
    mesh.add_color_attribute(
        "Col",
        Domain::Point,
        (0..10)
            .map(|point| Vec4::new(point as f32 / 10.0, 0.2, 0.3, 1.0))
            .collect(),
    );
    mesh.add_color_attribute("Paint", Domain::Corner, vec![Vec4::splat(0.5); corners]);
    mesh.attributes.insert(Attribute::new(
        "heat",
        Domain::Face,
        Values::Float(vec![1.5, 2.5]),
    ));
    mesh.attributes.insert(Attribute::new(
        "ids",
        Domain::Edge,
        Values::Int((0..mesh.edge_count() as i32).collect()),
    ));
    mesh.attributes.insert(Attribute::new(
        "flag",
        Domain::Corner,
        Values::Bool((0..corners).map(|corner| corner % 2 == 0).collect()),
    ));
    mesh.edge_creases_mut()[1] = 0.75;
    mesh.vertex_creases_mut()[3] = 0.25;
    mesh.uv_seams_mut()[0] = true;
    mesh.sharp_edges_mut()[2] = true;
    mesh.sharp_faces_mut()[1] = true;
    mesh.material_indices_mut()[1] = 3;
    mesh.selected_points_mut()[4] = true;
    mesh.hidden_faces_mut()[0] = true;
    mesh.vertex_groups.groups = vec![VertexGroup::new("Bone"), VertexGroup::new("Other")];
    mesh.vertex_groups.active = Some(1);
    mesh.vertex_groups.ensure_weights(10);
    mesh.vertex_groups.set_weight(1, 0, 0.5);
    mesh.vertex_groups.set_weight(6, 1, 1.0);
    let basis = mesh.positions.clone();
    let lifted: Vec<Vec3> = basis.iter().map(|&point| point + Vec3::Z).collect();
    mesh.shape_keys.add(ShapeKey::new("Basis", basis));
    let mut key = ShapeKey::new("Lift", lifted);
    key.value = 0.5;
    mesh.shape_keys.add(key);
    mesh.shape_keys.active = Some(0);
    mesh
}

#[test]
fn round_trip_preserves_everything() {
    let original = rich_mesh();
    original.validate().expect("valid fixture");
    let edit = EditMesh::from_polymesh(&original).expect("convert");
    checked(&edit);
    assert_eq!(edit.vert_count(), 10);
    assert_eq!(edit.face_count(), 2);
    assert!(edit.is_vert_loose(VertId::new(9)));
    assert_eq!(edit.to_polymesh(), original);
}

#[test]
fn round_trip_survives_edits_and_compaction() {
    let original = rich_mesh();
    let mut edit = EditMesh::from_polymesh(&original).expect("convert");
    let spare = edit.add_vert(Vec3::new(9.0, 9.0, 9.0));
    edit.kill_vert(spare).expect("kill");
    assert!(!edit.is_compact());
    let before = edit.to_polymesh();
    let compaction = edit.compact();
    assert_eq!(compaction.map_vert(VertId::new(3)), Some(VertId::new(3)));
    assert!(edit.is_compact());
    checked(&edit);
    assert_eq!(edit.to_polymesh(), before);
}

fn keyed() -> Polymesh {
    let mut mesh = Polymesh::from_polygons(
        vec![Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0), Vec3::Y],
        [vec![0, 1, 2, 3]],
    );
    let basis = mesh.positions.clone();
    let offset = |by: Vec3| -> Vec<Vec3> { basis.iter().map(|&point| point + by).collect() };
    mesh.shape_keys.add(ShapeKey::new("Basis", basis.clone()));
    mesh.shape_keys.add(ShapeKey::new("Up", offset(Vec3::Z)));
    let mut child = ShapeKey::new("UpMore", offset(Vec3::Z * 2.0));
    child.relative_to = 1;
    mesh.shape_keys.add(child);
    mesh.shape_keys.add(ShapeKey::new("Side", offset(Vec3::X)));
    mesh
}

#[test]
fn editing_active_key_offsets_dependents() {
    let mut source = keyed();
    source.shape_keys.active = Some(1);
    let mut edit = EditMesh::from_polymesh(&source).expect("convert");
    assert_eq!(edit.position(VertId::new(0)), Vec3::Z);
    edit.set_position(VertId::new(0), Vec3::new(0.0, 0.0, 3.0));
    let written = edit.to_polymesh();
    assert_eq!(written.positions[0], Vec3::ZERO);
    assert_eq!(
        written.shape_keys.keys[1].positions[0],
        Vec3::new(0.0, 0.0, 3.0)
    );
    assert_eq!(
        written.shape_keys.keys[2].positions[0],
        Vec3::new(0.0, 0.0, 4.0)
    );
    assert_eq!(written.shape_keys.keys[3].positions[0], Vec3::X);
    assert_eq!(written.shape_keys.keys[0].positions[0], Vec3::ZERO);
}

#[test]
fn editing_basis_propagates_until_new_vertex() {
    let mut source = keyed();
    source.shape_keys.active = Some(0);
    let mut edit = EditMesh::from_polymesh(&source).expect("convert");
    edit.set_position(VertId::new(1), Vec3::new(2.0, 0.0, 0.0));
    let written = edit.to_polymesh();
    assert_eq!(written.positions[1], Vec3::new(2.0, 0.0, 0.0));
    assert_eq!(
        written.shape_keys.keys[1].positions[1],
        Vec3::new(2.0, 0.0, 1.0)
    );
    assert_eq!(
        written.shape_keys.keys[2].positions[1],
        Vec3::new(2.0, 0.0, 2.0)
    );
    assert_eq!(
        written.shape_keys.keys[3].positions[1],
        Vec3::new(3.0, 0.0, 0.0)
    );

    let target = edit
        .edge_between(VertId::new(1), VertId::new(2))
        .expect("edge");
    let split = edit.split_edge(target, VertId::new(1), 0.5).expect("split");
    assert!(!edit.is_vert_original(split.vert));
    edit.set_position(VertId::new(0), Vec3::new(-1.0, 0.0, 0.0));
    let written = edit.to_polymesh();
    assert_eq!(written.positions[0], Vec3::new(-1.0, 0.0, 0.0));
    assert_eq!(written.shape_keys.keys[1].positions[0], Vec3::Z);
    assert!(
        (written.shape_keys.keys[1].positions[4] - Vec3::new(1.0, 0.5, 1.0)).length() < TOLERANCE
    );
}

#[test]
fn million_quad_grid_converts_and_updates_normals() {
    const SIDE: usize = 1000;
    let positions: Vec<Vec3> = (0..=SIDE)
        .flat_map(|row| (0..=SIDE).map(move |column| Vec3::new(column as f32, row as f32, 0.0)))
        .collect();
    let at = |column: usize, row: usize| (row * (SIDE + 1) + column) as u32;
    let polygons = (0..SIDE).flat_map(|row| {
        (0..SIDE).map(move |column| {
            [
                at(column, row),
                at(column + 1, row),
                at(column + 1, row + 1),
                at(column, row + 1),
            ]
        })
    });
    let mesh = Polymesh::from_polygons(positions, polygons);
    let started = Instant::now();
    let mut edit = EditMesh::from_polymesh(&mesh).expect("convert");
    let converted = started.elapsed();
    assert_eq!(edit.face_count(), SIDE * SIDE);
    let started = Instant::now();
    edit.update_normals();
    let normals = started.elapsed();
    assert_eq!(edit.vert_normal(VertId::new(12345)), Vec3::Z);
    let started = Instant::now();
    let moved: Vec<VertId> = (0..1000).map(|index| VertId::new(index * 997)).collect();
    for &vert in &moved {
        edit.set_position(vert, edit.position(vert) + Vec3::Z * 0.1);
    }
    edit.update_normals_around(moved.iter().copied());
    let partial = started.elapsed();
    let started = Instant::now();
    let tessellation = edit.tessellate();
    let tessellated = started.elapsed();
    assert_eq!(tessellation.triangles.len(), 2 * SIDE * SIDE);
    let started = Instant::now();
    let back = edit.to_polymesh();
    let written = started.elapsed();
    assert_eq!(back.face_count(), SIDE * SIDE);
    eprintln!(
        "1M quads: from_polymesh {converted:?}, update_normals {normals:?}, partial {partial:?}, tessellate {tessellated:?}, to_polymesh {written:?}"
    );
}
