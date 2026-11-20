use glam::{Vec2, Vec3};
use rikka_mesh_edit::{EdgeId, EditMesh, Element, FaceId, IdMap, SelectMode, VertId};
use rikka_polymesh::Polymesh;

pub(crate) const TOLERANCE: f32 = 2.0e-5;

pub(crate) struct Mesh {
    pub(crate) verts: &'static [[f32; 3]],
    pub(crate) edges: &'static [[u32; 2]],
    pub(crate) faces: &'static [&'static [u32]],
    pub(crate) uvs: &'static [[f32; 2]],
    pub(crate) weights: &'static [f32],
}

pub(crate) struct Selection {
    pub(crate) verts: &'static [u32],
    pub(crate) edges: &'static [u32],
    pub(crate) faces: &'static [u32],
}

pub(crate) struct Outcome {
    pub(crate) mesh: Mesh,
    pub(crate) selection: Selection,
}

pub(crate) fn checked(mesh: &EditMesh) {
    if let Err(error) = mesh.validate() {
        panic!("invalid edit mesh: {error}");
    }
}

pub(crate) fn edit_mesh(fixture: &Mesh, mode: SelectMode) -> EditMesh {
    let mut offsets = vec![0u32];
    let mut corners = Vec::new();
    for face in fixture.faces {
        corners.extend_from_slice(face);
        offsets.push(corners.len() as u32);
    }
    let mut polymesh = Polymesh {
        positions: fixture
            .verts
            .iter()
            .copied()
            .map(Vec3::from_array)
            .collect(),
        edges: fixture.edges.to_vec(),
        face_offsets: offsets,
        corner_verts: corners,
        ..Polymesh::default()
    };
    polymesh.ensure_edges();
    let mut mesh = EditMesh::from_polymesh(&polymesh).expect("fixture converts");
    if !fixture.uvs.is_empty() {
        mesh.add_uv_map("UVMap").expect("uv map");
        let loops: Vec<_> = mesh
            .faces()
            .flat_map(|face| mesh.face_loops_vec(face))
            .collect();
        let uvs = mesh.loop_uvs_mut("UVMap").expect("uv map");
        for (lp, value) in loops.iter().zip(fixture.uvs) {
            uvs[lp.index()] = Vec2::from_array(*value);
        }
    }
    if !fixture.weights.is_empty() {
        let group = mesh.add_vertex_group("Group");
        let verts: Vec<VertId> = mesh.verts().collect();
        for (vert, &weight) in verts.into_iter().zip(fixture.weights) {
            if weight != 0.0 {
                mesh.set_vert_weight(vert, group, weight);
            }
        }
        mesh.set_active_vertex_group(Some(group as usize));
    }
    mesh.set_select_mode(mode);
    mesh.update_normals();
    checked(&mesh);
    mesh
}

pub(crate) fn vert(mesh: &EditMesh, index: u32) -> VertId {
    mesh.verts().nth(index as usize).expect("vertex exists")
}

pub(crate) fn edge(mesh: &EditMesh, index: u32) -> EdgeId {
    mesh.edges().nth(index as usize).expect("edge exists")
}

pub(crate) fn face(mesh: &EditMesh, index: u32) -> FaceId {
    mesh.faces().nth(index as usize).expect("face exists")
}

#[derive(Default)]
pub(crate) struct Pick {
    pub(crate) verts: &'static [u32],
    pub(crate) edges: &'static [u32],
    pub(crate) faces: &'static [u32],
    pub(crate) history: &'static [u32],
}

pub(crate) fn select(mesh: &mut EditMesh, pick: Pick) {
    let verts: Vec<VertId> = pick.verts.iter().map(|&index| vert(mesh, index)).collect();
    let edges: Vec<EdgeId> = pick.edges.iter().map(|&index| edge(mesh, index)).collect();
    let faces: Vec<FaceId> = pick.faces.iter().map(|&index| face(mesh, index)).collect();
    let history: Vec<VertId> = pick
        .history
        .iter()
        .map(|&index| vert(mesh, index))
        .collect();
    mesh.deselect_all();
    for vert in verts {
        mesh.select_vert(vert, true);
    }
    for edge in edges {
        mesh.select_edge(edge, true);
    }
    for face in faces {
        mesh.select_face(face, true);
    }
    mesh.select_mode_flush();
    for vert in history {
        mesh.history_store(Element::Vert(vert));
    }
}

pub(crate) fn all_verts(mesh: &EditMesh) -> Vec<u32> {
    (0..mesh.vert_count() as u32).collect()
}

struct Indices {
    verts: IdMap<VertId, u32>,
}

impl Indices {
    fn new(mesh: &EditMesh) -> Indices {
        Indices {
            verts: mesh
                .verts()
                .enumerate()
                .map(|(index, vert)| (vert, index as u32))
                .collect(),
        }
    }

    fn vert(&self, vert: VertId) -> u32 {
        self.verts.copied(vert).expect("vertex is alive")
    }
}

pub(crate) fn positions(mesh: &EditMesh) -> Vec<Vec3> {
    mesh.verts().map(|vert| mesh.position(vert)).collect()
}

pub(crate) fn edge_list(mesh: &EditMesh) -> Vec<[u32; 2]> {
    let indices = Indices::new(mesh);
    mesh.edges()
        .map(|edge| mesh.edge_verts(edge).map(|vert| indices.vert(vert)))
        .collect()
}

pub(crate) fn face_list(mesh: &EditMesh) -> Vec<Vec<u32>> {
    let indices = Indices::new(mesh);
    mesh.faces()
        .map(|face| {
            mesh.face_verts(face)
                .map(|vert| indices.vert(vert))
                .collect()
        })
        .collect()
}

pub(crate) fn corner_uvs(mesh: &EditMesh) -> Vec<Vec2> {
    let Some(uvs) = mesh.loop_uvs("UVMap") else {
        return Vec::new();
    };
    mesh.faces()
        .flat_map(|face| mesh.face_loops_vec(face))
        .map(|lp| uvs[lp.index()])
        .collect()
}

pub(crate) fn assert_close(actual: Vec3, expected: Vec3, what: &str) {
    assert!(
        (actual - expected).abs().max_element() <= TOLERANCE,
        "{what}: {actual} != {expected}"
    );
}

pub(crate) fn assert_mesh(mesh: &EditMesh, expected: &Mesh) {
    checked(mesh);
    let actual = positions(mesh);
    assert_eq!(actual.len(), expected.verts.len(), "vertex count");
    for (index, (&position, value)) in actual.iter().zip(expected.verts).enumerate() {
        assert_close(
            position,
            Vec3::from_array(*value),
            &format!("vertex {index}"),
        );
    }
    let faces = face_list(mesh);
    let expected_faces: Vec<Vec<u32>> = expected.faces.iter().map(|face| face.to_vec()).collect();
    assert_eq!(faces, expected_faces, "faces");
    assert_eq!(edge_list(mesh), expected.edges.to_vec(), "edges");
    if !expected.uvs.is_empty() {
        let uvs = corner_uvs(mesh);
        assert_eq!(uvs.len(), expected.uvs.len(), "uv count");
        for (index, (&uv, value)) in uvs.iter().zip(expected.uvs).enumerate() {
            let expected = Vec2::from_array(*value);
            assert!(
                (uv - expected).abs().max_element() <= TOLERANCE,
                "uv {index}: {uv} != {expected}"
            );
        }
    }
    if !expected.weights.is_empty() {
        let verts: Vec<VertId> = mesh.verts().collect();
        for (index, (vert, &weight)) in verts.into_iter().zip(expected.weights).enumerate() {
            let actual = mesh.vert_weight(vert, 0);
            let expected = weight;
            assert!(
                (actual - expected).abs() <= TOLERANCE,
                "weight {index}: {actual} != {expected}"
            );
        }
    }
}

pub(crate) fn assert_selection(mesh: &EditMesh, expected: &Selection) {
    let verts: Vec<u32> = mesh
        .verts()
        .enumerate()
        .filter(|&(_, vert)| mesh.is_vert_selected(vert))
        .map(|(index, _)| index as u32)
        .collect();
    let edges: Vec<u32> = mesh
        .edges()
        .enumerate()
        .filter(|&(_, edge)| mesh.is_edge_selected(edge))
        .map(|(index, _)| index as u32)
        .collect();
    let faces: Vec<u32> = mesh
        .faces()
        .enumerate()
        .filter(|&(_, face)| mesh.is_face_selected(face))
        .map(|(index, _)| index as u32)
        .collect();
    assert_eq!(verts, expected.verts, "selected vertices");
    assert_eq!(edges, expected.edges, "selected edges");
    assert_eq!(faces, expected.faces, "selected faces");
}

pub(crate) fn assert_outcome(mesh: &EditMesh, expected: &Outcome) {
    assert_mesh(mesh, &expected.mesh);
    assert_selection(mesh, &expected.selection);
}
