use glam::{Vec2, Vec3};
use rikka_mesh_edit::{EdgeId, EditMesh, Element, FaceId, LoopId, VertId};

use crate::oracle::{Built, Selected};

pub(crate) const TOLERANCE: f32 = 1.0e-4;

pub(crate) fn micro3(value: &[i32; 3]) -> Vec3 {
    Vec3::new(value[0] as f32, value[1] as f32, value[2] as f32) * 1.0e-6
}

pub(crate) fn micro2(value: &[i32; 2]) -> Vec2 {
    Vec2::new(value[0] as f32, value[1] as f32) * 1.0e-6
}

pub(crate) fn checked(mesh: &EditMesh) {
    if let Err(error) = mesh.validate() {
        panic!("invalid edit mesh: {error}");
    }
}

pub(crate) fn build(verts: &[[f32; 3]], faces: &[&[u32]], wires: &[[u32; 2]]) -> EditMesh {
    let mut mesh = EditMesh::new();
    let ids: Vec<VertId> = verts
        .iter()
        .map(|&position| mesh.add_vert(Vec3::from_array(position)))
        .collect();
    for face in faces {
        let corners: Vec<VertId> = face.iter().map(|&index| ids[index as usize]).collect();
        mesh.add_face(&corners).expect("fixture face");
    }
    for &[a, b] in wires {
        mesh.add_edge(ids[a as usize], ids[b as usize])
            .expect("fixture wire");
    }
    mesh.update_normals();
    checked(&mesh);
    mesh
}

pub(crate) fn grid_points(
    columns: u32,
    rows: u32,
    offset: [f32; 2],
) -> (Vec<[f32; 3]>, Vec<Vec<u32>>) {
    let mut verts = Vec::new();
    for row in 0..=rows {
        for column in 0..=columns {
            verts.push([column as f32 + offset[0], row as f32 + offset[1], 0.0]);
        }
    }
    let mut faces = Vec::new();
    for row in 0..rows {
        for column in 0..columns {
            let at = |c: u32, r: u32| r * (columns + 1) + c;
            faces.push(vec![
                at(column, row),
                at(column + 1, row),
                at(column + 1, row + 1),
                at(column, row + 1),
            ]);
        }
    }
    (verts, faces)
}

pub(crate) fn grid(columns: u32, rows: u32) -> EditMesh {
    let (verts, faces) = grid_points(columns, rows, [0.0, 0.0]);
    let faces: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
    build(&verts, &faces, &[])
}

pub(crate) fn vert(index: u32) -> VertId {
    VertId::new(index)
}

pub(crate) fn face(index: u32) -> FaceId {
    FaceId::new(index)
}

pub(crate) fn edge(mesh: &EditMesh, a: u32, b: u32) -> EdgeId {
    mesh.edge_between(vert(a), vert(b)).expect("fixture edge")
}

pub(crate) fn corner(mesh: &EditMesh, face_index: u32, vert_index: u32) -> LoopId {
    mesh.face_vert_share_loop(face(face_index), vert(vert_index))
        .expect("fixture corner")
}

pub(crate) fn pick(mesh: &mut EditMesh, elements: &[Element]) {
    for &element in elements {
        mesh.select_element(element, true);
        mesh.history_store(element);
    }
    if let Some(Element::Face(face)) = elements.last() {
        mesh.set_active_face(Some(*face));
    }
    mesh.select_mode_flush();
}

pub(crate) fn assert_selected(mesh: &EditMesh, expected: &Selected) {
    let verts: Vec<u32> = mesh
        .verts()
        .filter(|&vert| mesh.is_vert_selected(vert))
        .map(VertId::raw)
        .collect();
    let mut edges: Vec<[u32; 2]> = mesh
        .edges()
        .filter(|&edge| mesh.is_edge_selected(edge))
        .map(|edge| {
            let [a, b] = mesh.edge_verts(edge).map(VertId::raw);
            [a.min(b), a.max(b)]
        })
        .collect();
    edges.sort_unstable();
    let faces: Vec<u32> = mesh
        .faces()
        .filter(|&face| mesh.is_face_selected(face))
        .map(FaceId::raw)
        .collect();
    assert_eq!(verts, expected.verts, "selected verts");
    assert_eq!(edges, expected.edges, "selected edges");
    assert_eq!(faces, expected.faces, "selected faces");
}

pub(crate) fn assert_built(mesh: &EditMesh, expected: &Built) {
    checked(mesh);
    let polymesh = mesh.to_polymesh();
    let faces: Vec<Vec<u32>> = (0..polymesh.face_count())
        .map(|face| polymesh.face_verts(face).to_vec())
        .collect();
    let expected_faces: Vec<Vec<u32>> = expected.faces.iter().map(|face| face.to_vec()).collect();
    assert_eq!(faces, expected_faces, "faces");
    assert_eq!(
        polymesh.positions.len(),
        expected.verts.len(),
        "vertex count"
    );
    for (index, (position, expected)) in polymesh.positions.iter().zip(expected.verts).enumerate() {
        assert!(
            position.distance(micro3(expected)) < TOLERANCE,
            "vertex {index}: {position} != {expected:?}"
        );
    }
}
