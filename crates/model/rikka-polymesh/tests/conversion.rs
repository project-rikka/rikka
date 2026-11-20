use glam::{Vec2, Vec3, Vec4};
use rikka_model_core::{MaterialId, Mesh};
use rikka_polymesh::{ColorSource, Domain, FromMesh, Influence, Polymesh, ShapeKey, ToMesh};

use crate::support::{cube, points, prism, vec2, vec3};

fn blender_cube() -> Polymesh {
    let mut mesh = Polymesh::from_polygons(
        points(&[
            [-1.0, -1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, 1.0, 1.0],
            [1.0, -1.0, -1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, -1.0],
            [1.0, 1.0, 1.0],
        ]),
        [
            [0u32, 1, 3, 2],
            [2, 3, 7, 6],
            [6, 7, 5, 4],
            [4, 5, 1, 0],
            [2, 6, 4, 0],
            [7, 3, 1, 5],
        ],
    );
    mesh.add_uv_map(
        "UVMap",
        vec2(&[
            [0.375, 0.0],
            [0.625, 0.0],
            [0.625, 0.25],
            [0.375, 0.25],
            [0.375, 0.25],
            [0.625, 0.25],
            [0.625, 0.5],
            [0.375, 0.5],
            [0.375, 0.5],
            [0.625, 0.5],
            [0.625, 0.75],
            [0.375, 0.75],
            [0.375, 0.75],
            [0.625, 0.75],
            [0.625, 1.0],
            [0.375, 1.0],
            [0.125, 0.5],
            [0.375, 0.5],
            [0.375, 0.75],
            [0.125, 0.75],
            [0.625, 0.5],
            [0.875, 0.5],
            [0.875, 0.75],
            [0.625, 0.75],
        ]),
    );
    mesh
}

#[test]
fn tangents_match_blender_mikktspace() {
    let converted = blender_cube().to_mesh(&ToMesh::new("cube"));
    let expected = vec3(&[
        [-0.408_248, -0.408_248, 0.816_497],
        [0.408_248, 0.408_248, 0.816_497],
        [0.408_248, -0.408_248, 0.816_497],
        [-0.408_248, 0.408_248, 0.816_497],
        [-0.408_248, 0.408_248, 0.816_497],
        [0.408_248, -0.408_248, 0.816_497],
        [-0.617_212, -0.154_306, 0.771_518],
        [0.617_215, 0.154_301, 0.771_516],
        [0.617_215, 0.154_301, 0.771_516],
        [-0.617_212, -0.154_306, 0.771_518],
        [-0.617_215, 0.154_301, 0.771_516],
        [0.617_212, -0.154_306, 0.771_518],
        [0.617_212, -0.154_306, 0.771_518],
        [-0.617_215, 0.154_301, 0.771_516],
        [0.408_248, 0.408_248, 0.816_497],
        [-0.408_248, -0.408_248, 0.816_497],
        [0.816_497, 0.408_248, -0.408_248],
        [0.617_215, 0.154_301, 0.771_516],
        [0.617_212, -0.154_306, 0.771_518],
        [0.816_497, -0.408_248, -0.408_248],
        [-0.617_212, -0.154_306, 0.771_518],
        [-0.816_497, -0.408_248, -0.408_248],
        [-0.816_497, 0.408_248, -0.408_248],
        [-0.617_215, 0.154_301, 0.771_516],
    ]);
    let polygons = converted
        .mesh
        .polygons
        .as_ref()
        .expect("polygons are recorded");
    for (corner, expected) in expected.iter().enumerate() {
        let tangent = converted.mesh.vertices.tangents[polygons.corner_vertices[corner] as usize];
        assert!(
            tangent.truncate().abs_diff_eq(*expected, 1.0e-4),
            "corner {corner}: {tangent}"
        );
        assert_eq!(tangent.w, -1.0, "corner {corner} handedness");
    }
}

#[test]
fn submeshes_follow_material_slots() {
    let mut mesh = cube();
    mesh.material_indices_mut()
        .copy_from_slice(&[0, 1, 0, 1, 2, 2]);
    let converted = mesh
        .to_mesh(&ToMesh::new("cube").materials(vec![Some(MaterialId(4)), Some(MaterialId(5))]));
    assert_eq!(converted.slots, vec![0, 1, 2]);
    let materials: Vec<Option<MaterialId>> = converted
        .mesh
        .submeshes
        .iter()
        .map(|submesh| submesh.material)
        .collect();
    assert_eq!(
        materials,
        vec![
            Some(MaterialId(4)),
            Some(MaterialId(5)),
            Some(MaterialId(5))
        ]
    );
    assert!(
        converted
            .mesh
            .submeshes
            .iter()
            .all(|submesh| submesh.index_count == 12)
    );
    assert_eq!(
        converted.triangle_faces,
        vec![0, 0, 2, 2, 1, 1, 3, 3, 4, 4, 5, 5]
    );
    let polygons = converted
        .mesh
        .polygons
        .as_ref()
        .expect("polygons are recorded");
    assert_eq!(polygons.submeshes, vec![0, 1, 0, 1, 2, 2]);
    assert_eq!(polygons.offsets, mesh.face_offsets);
    assert_eq!(converted.mesh.vertices.len(), 8);
    assert!(converted.mesh.vertices.tangents.is_empty());
}

#[test]
fn skins_keep_four_strongest_normalized_influences() {
    let mut mesh = cube();
    for name in ["a", "b", "c", "d", "e"] {
        mesh.vertex_groups.add(name);
    }
    mesh.vertex_groups.ensure_weights(8);
    mesh.vertex_groups.weights[0] = [0.1, 0.4, 0.2, 0.3, 0.05]
        .iter()
        .enumerate()
        .map(|(group, &weight)| Influence {
            group: group as u32,
            weight,
        })
        .collect();
    let converted =
        mesh.to_mesh(&ToMesh::new("cube").joints(vec![Some(0), Some(1), Some(2), Some(3), None]));
    let vertex = converted
        .vertex_points
        .iter()
        .position(|&point| point == 0)
        .unwrap();
    let influences = converted.mesh.vertices.influences(vertex).unwrap();
    assert_eq!(influences.map(|(joint, _)| joint), [1, 3, 2, 0]);
    let weights = influences.map(|(_, weight)| weight);
    assert!((weights.iter().sum::<f32>() - 1.0).abs() < 1.0e-6);
    assert!((weights[0] - 0.4).abs() < 1.0e-6);
}

#[test]
fn shape_keys_become_morph_targets() {
    let mut mesh = cube();
    mesh.add_color_attribute("Col", Domain::Point, vec![Vec4::new(1.0, 0.0, 0.0, 1.0); 8]);
    let basis = mesh.positions.clone();
    let mut lifted = basis.clone();
    lifted[6].z += 1.0;
    mesh.shape_keys.add(ShapeKey::new("Basis", basis.clone()));
    mesh.shape_keys.add(ShapeKey::new("Lift", lifted.clone()));
    let mut muted = ShapeKey::new("Muted", lifted);
    muted.muted = true;
    mesh.shape_keys.add(muted);
    let converted = mesh.to_mesh(&ToMesh::new("cube").colors(ColorSource::Render));
    assert_eq!(converted.shape_keys, vec![1]);
    let target = &converted.mesh.morph_targets[0];
    assert_eq!(target.name, "Lift");
    let lifted_vertex = converted
        .vertex_points
        .iter()
        .position(|&point| point == 6)
        .unwrap() as u32;
    let slot = target
        .vertices
        .iter()
        .position(|&vertex| vertex == lifted_vertex)
        .unwrap();
    assert_eq!(target.positions[slot], Vec3::Z);
    assert_eq!(target.normals.len(), target.vertices.len());
    assert!(
        converted
            .mesh
            .vertices
            .colors
            .iter()
            .all(|color| *color == [255, 0, 0, 255])
    );
}

fn corner_keys(mesh: &Mesh) -> Vec<[[i64; 8]; 3]> {
    let key = |vertex: u32| {
        let vertex = vertex as usize;
        let position = mesh.vertices.positions[vertex];
        let normal = mesh.vertices.normals[vertex];
        let uv = mesh.vertices.uvs.get(vertex).copied().unwrap_or(Vec2::ZERO);
        let round = |value: f32, scale: f32| (value * scale).round() as i64;
        [
            round(position.x, 1.0e4),
            round(position.y, 1.0e4),
            round(position.z, 1.0e4),
            round(uv.x, 1.0e4),
            round(uv.y, 1.0e4),
            round(normal.x, 1.0e3),
            round(normal.y, 1.0e3),
            round(normal.z, 1.0e3),
        ]
    };
    let mut triangles: Vec<[[i64; 8]; 3]> = mesh
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|triangle| {
            let mut corners = triangle.map(key);
            let lowest = (0..3).min_by_key(|&slot| corners[slot]).unwrap();
            corners.rotate_left(lowest);
            corners
        })
        .collect();
    triangles.sort();
    triangles
}

#[test]
fn triangle_meshes_round_trip() {
    let mut flat = blender_cube();
    flat.sharp_faces_mut().fill(true);
    let mut smooth = prism(16);
    smooth.sharpen_by_angle(30.0_f32.to_radians(), true);
    let mut custom = cube();
    let tilted: Vec<Vec3> = custom
        .corner_normals()
        .iter()
        .map(|normal| (*normal + Vec3::X).normalize())
        .collect();
    custom.custom_normals_mut().copy_from_slice(&tilted);
    for (case, source) in [("flat", flat), ("smooth", smooth), ("custom", custom)] {
        let mut original = source.to_mesh(&ToMesh::new(case).tangents(false)).mesh;
        original.polygons = None;
        let rebuilt = Polymesh::from_mesh(&original, &FromMesh::new());
        rebuilt.validate().expect("rebuilt meshes are valid");
        let again = rebuilt.to_mesh(&ToMesh::new(case).tangents(false)).mesh;
        assert_eq!(corner_keys(&again), corner_keys(&original), "{case}");
        assert_eq!(rebuilt.point_count(), source.point_count(), "{case} points");
    }
}

#[test]
fn polygon_meshes_rebuild_exact_faces() {
    let mut source = cube();
    source.material_indices_mut()[4] = 1;
    let converted = source.to_mesh(&ToMesh::new("cube"));
    let rebuilt = Polymesh::from_mesh(&converted.mesh, &FromMesh::new());
    assert_eq!(rebuilt.positions, source.positions);
    assert_eq!(rebuilt.face_offsets, source.face_offsets);
    assert_eq!(rebuilt.corner_verts, source.corner_verts);
    assert_eq!(rebuilt.material_indices(), Some(&[0, 0, 0, 0, 1, 0][..]));
    assert_eq!(rebuilt.custom_normals(), None);
}

#[test]
fn skins_and_morphs_survive_conversion() {
    let mut source = cube();
    source.vertex_groups.add("Root");
    source.vertex_groups.add("Tip");
    source.vertex_groups.ensure_weights(8);
    for point in 0..8 {
        source
            .vertex_groups
            .set_weight(point, (point / 4) as u32, 1.0);
    }
    let mut lifted = source.positions.clone();
    lifted[6].z += 2.0;
    source
        .shape_keys
        .add(ShapeKey::new("Basis", source.positions.clone()));
    source.shape_keys.add(ShapeKey::new("Lift", lifted));
    let mut converted = source
        .to_mesh(&ToMesh::new("cube").joints(vec![Some(0), Some(1)]))
        .mesh;
    converted.polygons = None;
    let rebuilt = Polymesh::from_mesh(&converted, &FromMesh::new().joint_names(["Root", "Tip"]));
    assert_eq!(rebuilt.point_count(), 8);
    let tip = rebuilt.vertex_groups.index("Tip").unwrap();
    let raised = rebuilt
        .positions
        .iter()
        .position(|position| *position == Vec3::ONE)
        .unwrap();
    assert_eq!(rebuilt.vertex_groups.weight(raised, tip), 1.0);
    let lift = rebuilt
        .shape_keys
        .get("Lift")
        .expect("morph targets become keys");
    assert_eq!(lift.positions[raised], Vec3::new(1.0, 1.0, 3.0));
}
