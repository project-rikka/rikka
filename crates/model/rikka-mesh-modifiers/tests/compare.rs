use glam::{Vec2, Vec3};
use rikka_polymesh::{Domain, Polymesh, ordered};

use crate::support::build;

pub(crate) const POSITION_SCALE: f32 = 1.0e4;
pub(crate) const NORMAL_TOLERANCE: f32 = 2.0e-4;

pub(crate) struct MeshData {
    pub(crate) positions: &'static [[f32; 3]],
    pub(crate) edges: &'static [[u32; 2]],
    pub(crate) faces: &'static [&'static [u32]],
    pub(crate) uvs: &'static [[f32; 2]],
    pub(crate) materials: &'static [i32],
    pub(crate) groups: &'static [&'static [(u32, f32)]],
    pub(crate) seams: &'static [[u32; 2]],
    pub(crate) sharp_edges: &'static [[u32; 2]],
    pub(crate) creases: &'static [([u32; 2], f32)],
    pub(crate) bevel_weights: &'static [([u32; 2], f32)],
    pub(crate) flat_faces: &'static [usize],
}

type Key = [i64; 3];

fn key(position: Vec3) -> Key {
    (position * POSITION_SCALE).round().as_i64vec3().to_array()
}

fn uv_key(uv: Vec2) -> [i64; 2] {
    (uv * POSITION_SCALE).round().as_i64vec2().to_array()
}

impl MeshData {
    pub(crate) fn build(&self, groups: &[&str]) -> Polymesh {
        let mut mesh = build(
            self.positions.iter().copied().map(Vec3::from).collect(),
            self.edges,
            self.faces,
        );
        if !self.uvs.is_empty() {
            mesh.add_uv_map("UVMap", self.uvs.iter().copied().map(Vec2::from).collect());
            mesh.uv_maps.set_active("UVMap");
        }
        if !self.materials.is_empty() {
            mesh.material_indices_mut().copy_from_slice(self.materials);
        }
        for name in groups {
            mesh.vertex_groups.add(*name);
        }
        if !self.groups.is_empty() {
            mesh.vertex_groups.ensure_weights(mesh.point_count());
            for (point, row) in self.groups.iter().enumerate() {
                for &(group, weight) in row.iter() {
                    mesh.vertex_groups.set_weight(point, group, weight);
                }
            }
        }
        let edge_index = |mesh: &Polymesh, pair: [u32; 2]| {
            mesh.edges
                .iter()
                .position(|&edge| ordered(edge) == ordered(pair))
                .expect("flagged edge exists")
        };
        for &pair in self.seams {
            let edge = edge_index(&mesh, pair);
            mesh.uv_seams_mut()[edge] = true;
        }
        for &pair in self.sharp_edges {
            let edge = edge_index(&mesh, pair);
            mesh.sharp_edges_mut()[edge] = true;
        }
        for &(pair, value) in self.creases {
            let edge = edge_index(&mesh, pair);
            mesh.edge_creases_mut()[edge] = value;
        }
        for &(pair, value) in self.bevel_weights {
            let edge = edge_index(&mesh, pair);
            mesh.edge_bevel_weights_mut()[edge] = value;
        }
        for &face in self.flat_faces {
            mesh.sharp_faces_mut()[face] = true;
        }
        mesh
    }

    fn position(&self, point: u32) -> Vec3 {
        Vec3::from(self.positions[point as usize])
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FaceKey {
    corners: Vec<(Key, Option<[i64; 2]>)>,
    material: i32,
}

fn face_key(corners: Vec<(Key, Option<[i64; 2]>)>, material: i32) -> (FaceKey, usize) {
    let start = (0..corners.len())
        .min_by_key(|&slot| corners[slot])
        .unwrap_or(0);
    let mut rotated = corners;
    rotated.rotate_left(start);
    (
        FaceKey {
            corners: rotated,
            material,
        },
        start,
    )
}

fn mesh_faces(mesh: &Polymesh, uvs: bool) -> Vec<FaceKey> {
    let map = uvs.then(|| mesh.uv_map("UVMap").expect("the UV map survives"));
    let mut faces: Vec<FaceKey> = (0..mesh.face_count())
        .map(|face| {
            let corners = mesh
                .face(face)
                .map(|corner| {
                    (
                        key(mesh.positions[mesh.corner_verts[corner] as usize]),
                        map.map(|map| uv_key(map[corner])),
                    )
                })
                .collect();
            face_key(corners, mesh.material_index(face)).0
        })
        .collect();
    faces.sort();
    faces
}

fn expected_faces(expected: &MeshData) -> Vec<FaceKey> {
    let mut corner = 0;
    let mut faces: Vec<FaceKey> = expected
        .faces
        .iter()
        .enumerate()
        .map(|(face, points)| {
            let corners = points
                .iter()
                .map(|&point| {
                    let uv = (!expected.uvs.is_empty())
                        .then(|| uv_key(Vec2::from(expected.uvs[corner])));
                    corner += 1;
                    (key(expected.position(point)), uv)
                })
                .collect();
            face_key(corners, expected.materials.get(face).copied().unwrap_or(0)).0
        })
        .collect();
    faces.sort();
    faces
}

fn edge_keys(mesh: &Polymesh, flags: Option<&[bool]>) -> Vec<[Key; 2]> {
    let mut keys: Vec<[Key; 2]> = mesh
        .edges
        .iter()
        .enumerate()
        .filter(|&(edge, _)| flags.is_some_and(|flags| flags[edge]))
        .map(|(_, &[a, b])| {
            let mut pair = [
                key(mesh.positions[a as usize]),
                key(mesh.positions[b as usize]),
            ];
            pair.sort();
            pair
        })
        .collect();
    keys.sort();
    keys
}

fn expected_edge_keys(expected: &MeshData, pairs: &[[u32; 2]]) -> Vec<[Key; 2]> {
    let mut keys: Vec<[Key; 2]> = pairs
        .iter()
        .map(|&[a, b]| {
            let mut pair = [key(expected.position(a)), key(expected.position(b))];
            pair.sort();
            pair
        })
        .collect();
    keys.sort();
    keys
}

fn edge_values(mesh: &Polymesh, values: Option<&[f32]>) -> Vec<([Key; 2], i64)> {
    let mut keys: Vec<([Key; 2], i64)> = mesh
        .edges
        .iter()
        .enumerate()
        .filter_map(|(edge, &[a, b])| {
            let value = values?[edge];
            (value != 0.0).then(|| {
                let mut pair = [
                    key(mesh.positions[a as usize]),
                    key(mesh.positions[b as usize]),
                ];
                pair.sort();
                (pair, (value * POSITION_SCALE).round() as i64)
            })
        })
        .collect();
    keys.sort();
    keys
}

fn expected_edge_values(expected: &MeshData, values: &[([u32; 2], f32)]) -> Vec<([Key; 2], i64)> {
    let mut keys: Vec<([Key; 2], i64)> = values
        .iter()
        .map(|&([a, b], value)| {
            let mut pair = [key(expected.position(a)), key(expected.position(b))];
            pair.sort();
            (pair, (value * POSITION_SCALE).round() as i64)
        })
        .collect();
    keys.sort();
    keys
}

pub(crate) fn assert_mesh(mesh: &Polymesh, expected: &MeshData) {
    assert_eq!(mesh.point_count(), expected.positions.len(), "point count");
    assert_eq!(mesh.edge_count(), expected.edges.len(), "edge count");
    assert_eq!(mesh.face_count(), expected.faces.len(), "face count");
    assert_eq!(
        mesh_faces(mesh, !expected.uvs.is_empty()),
        expected_faces(expected),
        "faces"
    );
    let mut edges = edge_keys(mesh, Some(&vec![true; mesh.edge_count()]));
    edges.dedup();
    let mut expected_edges = expected_edge_keys(expected, expected.edges);
    expected_edges.dedup();
    assert_eq!(edges, expected_edges, "edges");
    assert_eq!(
        edge_keys(mesh, mesh.uv_seams()),
        expected_edge_keys(expected, expected.seams),
        "seams"
    );
    assert_eq!(
        edge_keys(mesh, mesh.sharp_edges()),
        expected_edge_keys(expected, expected.sharp_edges),
        "sharp edges"
    );
    assert_eq!(
        edge_values(mesh, mesh.edge_creases()),
        expected_edge_values(expected, expected.creases),
        "creases"
    );
    assert_eq!(
        edge_values(mesh, mesh.edge_bevel_weights()),
        expected_edge_values(expected, expected.bevel_weights),
        "bevel weights"
    );
    let mut flat: Vec<Vec<(Key, Option<[i64; 2]>)>> = (0..mesh.face_count())
        .filter(|&face| mesh.is_sharp_face(face))
        .map(|face| {
            let corners = mesh
                .face_verts(face)
                .iter()
                .map(|&point| (key(mesh.positions[point as usize]), None))
                .collect();
            face_key(corners, 0).0.corners
        })
        .collect();
    flat.sort();
    let mut expected_flat: Vec<Vec<(Key, Option<[i64; 2]>)>> = expected
        .flat_faces
        .iter()
        .map(|&face| {
            let corners = expected.faces[face]
                .iter()
                .map(|&point| (key(expected.position(point)), None))
                .collect();
            face_key(corners, 0).0.corners
        })
        .collect();
    expected_flat.sort();
    assert_eq!(flat, expected_flat, "flat faces");
    if !expected.groups.is_empty() {
        let mut groups: Vec<(Key, Vec<(u32, i64)>)> = (0..mesh.point_count())
            .map(|point| {
                let mut row: Vec<(u32, i64)> = mesh
                    .vertex_groups
                    .weights
                    .get(point)
                    .map(|row| {
                        row.iter()
                            .filter(|influence| influence.weight != 0.0)
                            .map(|influence| {
                                (
                                    influence.group,
                                    (influence.weight * POSITION_SCALE).round() as i64,
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                row.sort();
                (key(mesh.positions[point]), row)
            })
            .collect();
        groups.sort();
        let mut expected_groups: Vec<(Key, Vec<(u32, i64)>)> = expected
            .groups
            .iter()
            .enumerate()
            .map(|(point, row)| {
                let mut row: Vec<(u32, i64)> = row
                    .iter()
                    .filter(|&&(_, weight)| weight != 0.0)
                    .map(|&(group, weight)| (group, (weight * POSITION_SCALE).round() as i64))
                    .collect();
                row.sort();
                (key(expected.position(point as u32)), row)
            })
            .collect();
        expected_groups.sort();
        assert_eq!(groups, expected_groups, "vertex groups");
    }
}

pub(crate) fn assert_order(mesh: &Polymesh, expected: &MeshData) {
    assert_eq!(mesh.point_count(), expected.positions.len(), "point count");
    for (point, (actual, wanted)) in mesh.positions.iter().zip(expected.positions).enumerate() {
        assert!(
            actual.abs_diff_eq(Vec3::from(*wanted), 1.0e-5),
            "point {point}: {actual} != {wanted:?}"
        );
    }
    let edges: Vec<[u32; 2]> = mesh.edges.iter().map(|&edge| ordered(edge)).collect();
    let wanted: Vec<[u32; 2]> = expected.edges.iter().map(|&edge| ordered(edge)).collect();
    assert_eq!(edges, wanted, "edge order");
    let faces: Vec<Vec<u32>> = (0..mesh.face_count())
        .map(|face| mesh.face_verts(face).to_vec())
        .collect();
    let wanted: Vec<Vec<u32>> = expected.faces.iter().map(|face| face.to_vec()).collect();
    assert_eq!(faces, wanted, "face order");
    if !expected.uvs.is_empty() {
        let uvs = mesh.uv_map("UVMap").expect("the UV map survives");
        assert_eq!(uvs.len(), expected.uvs.len(), "corner count");
        for (corner, (actual, wanted)) in uvs.iter().zip(expected.uvs).enumerate() {
            assert!(
                actual.abs_diff_eq(Vec2::from(*wanted), 1.0e-5),
                "corner {corner}: {actual} != {wanted:?}"
            );
        }
    }
    let materials: Vec<i32> = (0..mesh.face_count())
        .map(|face| mesh.material_index(face))
        .collect();
    let wanted: Vec<i32> = match expected.materials.is_empty() {
        true => vec![0; expected.faces.len()],
        false => expected.materials.to_vec(),
    };
    assert_eq!(materials, wanted, "materials");
}

pub(crate) fn assert_corner_normals(
    mesh: &Polymesh,
    expected: &MeshData,
    expected_normals: &[[f32; 3]],
) {
    let normals = mesh.corner_normals();
    let mut actual: Vec<(FaceKey, usize, Vec3)> = Vec::new();
    for face in 0..mesh.face_count() {
        let corners: Vec<(Key, Option<[i64; 2]>)> = mesh
            .face_verts(face)
            .iter()
            .map(|&point| (key(mesh.positions[point as usize]), None))
            .collect();
        let (face_key, start) = face_key(corners, 0);
        let range = mesh.face(face);
        let len = range.len();
        for slot in 0..len {
            actual.push((
                face_key.clone(),
                slot,
                normals[range.start + (slot + start) % len],
            ));
        }
    }
    let mut wanted: Vec<(FaceKey, usize, Vec3)> = Vec::new();
    let mut corner = 0;
    for points in expected.faces {
        let corners: Vec<(Key, Option<[i64; 2]>)> = points
            .iter()
            .map(|&point| (key(expected.position(point)), None))
            .collect();
        let (face_key, start) = face_key(corners, 0);
        let len = points.len();
        for slot in 0..len {
            wanted.push((
                face_key.clone(),
                slot,
                Vec3::from(expected_normals[corner + (slot + start) % len]),
            ));
        }
        corner += len;
    }
    let order =
        |a: &(FaceKey, usize, Vec3), b: &(FaceKey, usize, Vec3)| (&a.0, a.1).cmp(&(&b.0, b.1));
    actual.sort_by(order);
    wanted.sort_by(order);
    assert_eq!(actual.len(), wanted.len(), "corner count");
    for (actual, wanted) in actual.iter().zip(&wanted) {
        assert_eq!(actual.0, wanted.0, "corner faces");
        assert!(
            actual.2.abs_diff_eq(wanted.2, NORMAL_TOLERANCE),
            "corner normal {} != {} on {:?}",
            actual.2,
            wanted.2,
            actual.0
        );
    }
}

pub(crate) fn assert_face_values(
    mesh: &Polymesh,
    name: &str,
    expected: &MeshData,
    expected_values: &[i32],
) {
    let values = mesh
        .attributes
        .values::<i32>(name, Domain::Face)
        .expect("face attribute exists");
    let mut actual: Vec<(FaceKey, i32)> = (0..mesh.face_count())
        .map(|face| {
            let corners = mesh
                .face_verts(face)
                .iter()
                .map(|&point| (key(mesh.positions[point as usize]), None))
                .collect();
            (face_key(corners, 0).0, values[face])
        })
        .collect();
    let mut wanted: Vec<(FaceKey, i32)> = expected
        .faces
        .iter()
        .zip(expected_values)
        .map(|(points, &value)| {
            let corners = points
                .iter()
                .map(|&point| (key(expected.position(point)), None))
                .collect();
            (face_key(corners, 0).0, value)
        })
        .collect();
    actual.sort();
    wanted.sort();
    assert_eq!(actual, wanted, "{name}");
}
