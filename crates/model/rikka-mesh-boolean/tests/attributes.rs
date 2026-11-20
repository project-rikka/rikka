use glam::{Vec2, Vec3};
use rikka_mesh_boolean::{Boolean, MaterialMode, Operand, Operation, Solver, material_remap};
use rikka_polymesh::{Domain, Polymesh};

use crate::support::{cube, moved};

fn decorated_cube() -> Polymesh {
    let mut mesh = cube();
    let group = mesh.vertex_groups.add("x");
    mesh.vertex_groups.ensure_weights(mesh.point_count());
    for point in 0..mesh.point_count() {
        let weight = (mesh.positions[point].x + 1.0) * 0.5;
        mesh.vertex_groups.set_weight(point, group, weight);
    }
    mesh.edge_creases_mut().fill(0.7);
    let faces = mesh.face_count();
    for (face, material) in mesh
        .material_indices_mut()
        .iter_mut()
        .enumerate()
        .take(faces)
    {
        *material = (face % 3) as i32;
    }
    mesh
}

fn expected_uv(source: &Polymesh, face: usize, point: Vec3) -> Vec2 {
    let corners = source.face(face);
    let uvs = source.active_uv_map().unwrap();
    let p0 = source.positions[source.corner_verts[corners.start] as usize];
    let p1 = source.positions[source.corner_verts[corners.start + 1] as usize];
    let p3 = source.positions[source.corner_verts[corners.start + 3] as usize];
    let (d1, d3) = (p1 - p0, p3 - p0);
    let s = (point - p0).dot(d1) / d1.length_squared();
    let t = (point - p0).dot(d3) / d3.length_squared();
    let (uv0, uv1, uv3) = (
        uvs[corners.start],
        uvs[corners.start + 1],
        uvs[corners.start + 3],
    );
    uv0 + (uv1 - uv0) * s + (uv3 - uv0) * t
}

#[test]
fn attributes_follow_cut_faces() {
    let a = decorated_cube();
    let b = cube();
    for solver in [Solver::Exact, Solver::Fast] {
        let result = Boolean::new(Operation::Difference)
            .operand(&a)
            .operand(
                Operand::new(&b)
                    .transform(moved([1.0, 0.5, 0.25]))
                    .material_remap([4]),
            )
            .solver(solver)
            .compute()
            .unwrap();
        let mesh = &result.mesh;
        mesh.validate().unwrap();
        assert_eq!(result.face_origins.len(), mesh.face_count());
        let uvs = mesh.active_uv_map().unwrap();
        let group = mesh.vertex_groups.index("x").unwrap();
        let materials = mesh.material_indices().unwrap();
        let mut from_a = vec![false; mesh.point_count()];
        for face in 0..mesh.face_count() {
            let origin = result.face_origins[face];
            match origin.operand {
                0 => {
                    assert_eq!(
                        materials[face],
                        (origin.face % 3) as i32,
                        "{solver:?} material"
                    );
                    for corner in mesh.face(face) {
                        let point = mesh.corner_verts[corner] as usize;
                        from_a[point] = true;
                        let expected = expected_uv(&a, origin.face as usize, mesh.positions[point]);
                        assert!(
                            uvs[corner].distance(expected) < 1.0e-5,
                            "{solver:?} uv {:?} vs {expected:?} at {:?} in face {face} from {origin:?} corners {:?}",
                            uvs[corner],
                            mesh.positions[point],
                            mesh.face(face)
                                .map(|c| (mesh.positions[mesh.corner_verts[c] as usize], uvs[c]))
                                .collect::<Vec<_>>()
                        );
                    }
                }
                1 => assert_eq!(materials[face], 4, "{solver:?} remapped material"),
                other => panic!("unexpected operand {other}"),
            }
        }
        for point in 0..mesh.point_count() {
            let weight = mesh.vertex_groups.weight(point, group);
            if from_a[point] && solver == Solver::Exact {
                let expected = (mesh.positions[point].x + 1.0) * 0.5;
                assert!(
                    (weight - expected).abs() < 1.0e-5,
                    "weight {weight} vs {expected}"
                );
            }
        }
        let creases = mesh.edge_creases().unwrap();
        for (edge, pair) in mesh.edges.iter().enumerate() {
            let (p, q) = (
                mesh.positions[pair[0] as usize],
                mesh.positions[pair[1] as usize],
            );
            let on_cube_edge = (0..3)
                .filter(|&axis| {
                    (p[axis].abs() - 1.0).abs() < 1.0e-6 && (p[axis] - q[axis]).abs() < 1.0e-6
                })
                .count()
                >= 2;
            assert_eq!(
                creases[edge] == 0.7,
                on_cube_edge,
                "{solver:?} crease on {p:?} {q:?}"
            );
        }
    }
}

#[test]
fn untouched_quads_stay_quads() {
    let a = decorated_cube();
    let b = cube();
    let result = Boolean::new(Operation::Union)
        .operand(&a)
        .operand(Operand::new(&b).transform(moved([1.0, 0.5, 0.25])))
        .compute()
        .unwrap();
    let mesh = &result.mesh;
    let source_uvs = a.active_uv_map().unwrap();
    let uvs = mesh.active_uv_map().unwrap();
    let minus_x = (0..a.face_count())
        .find(|&face| {
            a.face_verts(face)
                .iter()
                .all(|&v| a.positions[v as usize].x == -1.0)
        })
        .unwrap();
    let face = (0..mesh.face_count())
        .find(|&face| {
            result.face_origins[face].operand == 0
                && result.face_origins[face].face == minus_x as u32
        })
        .unwrap();
    assert_eq!(mesh.face_size(face), 4);
    let mut expected: Vec<(Vec3, Vec2)> = a
        .face(minus_x)
        .map(|c| (a.positions[a.corner_verts[c] as usize], source_uvs[c]))
        .collect();
    let mut actual: Vec<(Vec3, Vec2)> = mesh
        .face(face)
        .map(|c| (mesh.positions[mesh.corner_verts[c] as usize], uvs[c]))
        .collect();
    let key = |item: &(Vec3, Vec2)| (item.0.y, item.0.z);
    expected.sort_by(|x, y| key(x).partial_cmp(&key(y)).unwrap());
    actual.sort_by(|x, y| key(x).partial_cmp(&key(y)).unwrap());
    assert_eq!(expected, actual);
    let flipped_faces = (0..mesh.face_count())
        .filter(|&face| result.face_origins[face].operand == 1)
        .count();
    assert!(flipped_faces > 0);
    assert!(
        mesh.attributes
            .values::<i32>("material_index", Domain::Face)
            .is_some()
    );
}

#[test]
fn intersecting_edges_trace_the_cut() {
    let result = Boolean::new(Operation::Union)
        .operand(&cube())
        .operand(Operand::new(&cube()).transform(moved([1.0, 0.5, 0.25])))
        .compute()
        .unwrap();
    assert_eq!(result.intersecting_edges.len(), 6);
    let mesh = &result.mesh;
    for &edge in &result.intersecting_edges {
        let [a, b] = mesh.edges[edge as usize];
        let (p, q) = (mesh.positions[a as usize], mesh.positions[b as usize]);
        let inside_both = |point: Vec3| {
            point.abs().max_element() <= 1.0 + 1.0e-6
                && (point - Vec3::new(1.0, 0.5, 0.25)).abs().max_element() <= 1.0 + 1.0e-6
        };
        assert!(inside_both(p) && inside_both(q));
    }
}

#[test]
fn material_modes_build_slot_tables() {
    let mut slots = vec![Some("steel"), Some("wood")];
    let operand = [Some("wood"), Some("glass"), Some("steel")];
    assert_eq!(
        material_remap(MaterialMode::IndexBased, &mut slots, &operand),
        vec![1, 1, 0]
    );
    assert_eq!(slots, vec![Some("steel"), Some("wood")]);
    let operand = [Some("wood"), Some("glass"), None];
    assert_eq!(
        material_remap(MaterialMode::Transfer, &mut slots, &operand),
        vec![1, 2, -1]
    );
    assert_eq!(slots, vec![Some("steel"), Some("wood"), Some("glass")]);
    let mut empty: Vec<Option<&str>> = Vec::new();
    assert_eq!(
        material_remap(MaterialMode::Transfer, &mut empty, &[Some("glass")]),
        vec![1]
    );
    assert_eq!(empty, vec![None, Some("glass")]);
    assert_eq!(
        material_remap::<&str>(MaterialMode::IndexBased, &mut empty, &[]),
        vec![0]
    );
}

#[test]
fn material_tables_apply_per_operand() {
    let a = cube();
    let mut b = cube();
    for (face, material) in b.material_indices_mut().iter_mut().enumerate() {
        *material = (face % 3) as i32;
    }
    let mut slots = vec![Some("steel")];
    let table = material_remap(
        MaterialMode::Transfer,
        &mut slots,
        &[Some("wood"), Some("glass"), Some("steel")],
    );
    for solver in [Solver::Exact, Solver::Fast] {
        let result = Boolean::new(Operation::Union)
            .operand(&a)
            .operand(
                Operand::new(&b)
                    .transform(moved([1.0, 0.5, 0.25]))
                    .material_remap(table.clone()),
            )
            .solver(solver)
            .compute()
            .unwrap();
        let materials = result.mesh.material_indices().unwrap();
        for (face, origin) in result.face_origins.iter().enumerate() {
            let expected = match origin.operand {
                0 => 0,
                _ => table[origin.face as usize % 3],
            };
            assert_eq!(materials[face], expected, "{solver:?} face {face}");
        }
    }
}
