use glam::Vec3;
use rikka_polymesh::{Attribute, Domain, Influence, Polymesh, ShapeKey, Values};

use crate::support::{cube, points};

fn broken(change: impl FnOnce(&mut Polymesh)) -> Polymesh {
    let mut mesh = cube();
    change(&mut mesh);
    mesh
}

#[test]
fn validation_rejects_broken_invariants() {
    let cases: Vec<(&str, Polymesh)> = vec![
        ("offset start", broken(|mesh| mesh.face_offsets[0] = 1)),
        ("short face", broken(|mesh| mesh.face_offsets[1] = 2)),
        ("missing point", broken(|mesh| mesh.corner_verts[0] = 99)),
        ("edge point", broken(|mesh| mesh.edges[0] = [0, 99])),
        ("corner edge", broken(|mesh| mesh.corner_edges.swap(0, 1))),
        (
            "attribute length",
            broken(|mesh| {
                mesh.attributes.insert(Attribute::new(
                    "heat",
                    Domain::Point,
                    Values::Float(vec![0.0; 3]),
                ));
            }),
        ),
        (
            "well-known type",
            broken(|mesh| {
                mesh.attributes.insert(Attribute::new(
                    "sharp_face",
                    Domain::Face,
                    Values::Float(vec![0.0; 6]),
                ));
            }),
        ),
        (
            "UV map",
            broken(|mesh| mesh.uv_maps.names.push("missing".into())),
        ),
        (
            "weights",
            broken(|mesh| mesh.vertex_groups.weights = vec![Vec::new(); 3]),
        ),
        (
            "weight group",
            broken(|mesh| {
                mesh.vertex_groups.weights = vec![
                    vec![Influence {
                        group: 4,
                        weight: 1.0
                    }];
                    8
                ];
            }),
        ),
        (
            "key relative",
            broken(|mesh| {
                let mut key = ShapeKey::new("Basis", mesh.positions.clone());
                key.relative_to = 3;
                mesh.shape_keys.add(key);
            }),
        ),
    ];
    for (case, mesh) in cases {
        assert!(mesh.validate().is_err(), "{case} was accepted");
    }
}

#[test]
fn loose_geometry_is_valid() {
    let mut mesh = Polymesh::from_polygons(
        points(&[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [5.0, 5.0, 5.0],
            [6.0, 5.0, 5.0],
            [9.0, 9.0, 9.0],
        ]),
        [[0u32, 1, 2]],
    );
    mesh.edges.push([3, 4]);
    mesh.validate().expect("loose points and edges are legal");
    assert_eq!(mesh.edge_count(), 4);
    assert_eq!(
        mesh.loose_points(),
        vec![false, false, false, false, false, true]
    );
    assert_eq!(mesh.loose_edges(), vec![false, false, false, true]);
}

#[test]
fn sanitize_repairs_hostile_data() {
    let mut mesh = cube();
    mesh.material_indices_mut()
        .copy_from_slice(&[0, 1, 2, 3, 4, 5]);
    mesh.add_uv_map(
        "UVMap",
        (0..24)
            .map(|corner| glam::Vec2::splat(corner as f32))
            .collect(),
    );
    mesh.corner_verts[4] = 1000;
    mesh.face_offsets.push(u32::MAX);
    mesh.edges.push([2, 77]);
    mesh.attributes.insert(Attribute::new(
        "heat",
        Domain::Point,
        Values::Float(vec![1.0; 2]),
    ));
    mesh.vertex_groups.add("Arm");
    mesh.vertex_groups.weights = vec![vec![Influence {
        group: 9,
        weight: 1.0,
    }]];
    mesh.shape_keys.add(ShapeKey::new("Basis", vec![Vec3::ONE]));
    mesh.uv_maps.names.push("gone".into());
    mesh.uv_maps.active = Some(7);

    assert!(mesh.sanitize());
    mesh.validate().expect("sanitized meshes are valid");
    assert_eq!(mesh.face_count(), 5);
    assert_eq!(mesh.material_indices(), Some(&[0, 2, 3, 4, 5][..]));
    assert_eq!(mesh.uv_map("UVMap").map(|uvs| uvs[4].x), Some(8.0));
    assert!(mesh.edges.iter().flatten().all(|&point| point < 8));
    assert_eq!(mesh.vertex_groups.weights.len(), 8);
    assert!(mesh.vertex_groups.weights.iter().all(Vec::is_empty));
    assert_eq!(mesh.shape_keys.keys[0].positions[1], mesh.positions[1]);
    assert_eq!(mesh.uv_maps.names, vec!["UVMap".to_owned()]);
    assert!(!mesh.sanitize());
}

#[test]
fn ensure_edges_joins_corners_and_keeps_edge_data() {
    let mut mesh = cube();
    assert_eq!(mesh.edge_count(), 12);
    mesh.edge_creases_mut()[0] = 1.0;
    let first = mesh.edges[0];
    mesh.corner_edges.iter_mut().for_each(|edge| *edge = 0);
    mesh.ensure_edges();
    mesh.validate().expect("edges were repaired");
    assert_eq!(mesh.edge_count(), 12);
    assert_eq!(mesh.edges[0], first);
    assert_eq!(mesh.edge_crease(0), 1.0);
}
