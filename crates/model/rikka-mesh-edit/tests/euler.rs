use rikka_mesh_edit::{DoubleFace, EditMesh, NgonMethod, QuadMethod, RotateChecks, Rotation};

use crate::oracle::{self, Built};
use crate::support::{assert_built, build, checked, corner, edge, face, grid, vert};

fn grid3() -> EditMesh {
    grid(3, 3)
}

fn pentagon() -> EditMesh {
    build(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        &[&[0, 1, 2, 3, 4]],
        &[],
    )
}

fn ngon() -> EditMesh {
    build(
        &[
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [3.0, 1.0, 0.0],
            [2.0, 2.0, 0.0],
            [1.0, 1.5, 0.0],
            [0.0, 2.0, 0.0],
            [-1.0, 1.0, 0.0],
        ],
        &[&[0, 1, 2, 3, 4, 5, 6]],
        &[],
    )
}

fn quad() -> EditMesh {
    build(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.2, 1.0, 0.0],
            [0.0, 0.9, 0.0],
        ],
        &[&[0, 1, 2, 3]],
        &[],
    )
}

#[test]
fn dissolve_vert_matches_blender() {
    let cases: [(u32, &Built); 2] = [
        (5, &oracle::DISSOLVE_INTERIOR),
        (1, &oracle::DISSOLVE_BOUNDARY),
    ];
    for (index, expected) in cases {
        let mut mesh = grid3();
        mesh.dissolve_vert(vert(index)).expect("dissolve");
        assert_built(&mesh, expected);
    }
}

#[test]
fn split_and_join_faces_match_blender() {
    let mut mesh = grid3();
    let (a, b) = (corner(&mesh, 4, 5), corner(&mesh, 4, 10));
    let split = mesh.split_face(face(4), a, b, None, true).expect("split");
    assert_eq!(mesh.loop_vert(split.lp), vert(5));
    assert_built(&mesh, &oracle::SPLIT_FACE);

    let mut mesh = grid3();
    let joined = mesh
        .join_faces(&[face(0), face(1), face(3), face(4)], DoubleFace::Reuse)
        .expect("join");
    assert_eq!(joined.double, None);
    assert_built(&mesh, &oracle::JOIN_FACES);
}

#[test]
fn edge_split_and_separation_match_blender() {
    let mut mesh = grid3();
    let target = edge(&mesh, 5, 6);
    mesh.split_edge(target, vert(5), 0.25).expect("split edge");
    assert_built(&mesh, &oracle::EDGE_SPLIT);

    let mut mesh = grid3();
    let edges = [edge(&mesh, 5, 6), edge(&mesh, 5, 9)];
    let verts = mesh
        .separate_vert(vert(5), &edges, false)
        .expect("separate");
    assert_eq!(verts.len(), 2);
    assert_built(&mesh, &oracle::VERT_SEPARATE);

    let mut mesh = grid3();
    let first = mesh.face_first_loop(face(4));
    mesh.separate_loop_region(first).expect("separate loop");
    assert_built(&mesh, &oracle::LOOP_SEPARATE);
}

#[test]
fn flip_and_rotate_match_blender() {
    let mut mesh = grid3();
    mesh.reverse_face(face(4)).expect("flip");
    assert_built(&mesh, &oracle::FACE_FLIP);

    for (rotation, expected) in [
        (Rotation::Clockwise, &oracle::ROTATE_CW),
        (Rotation::CounterClockwise, &oracle::ROTATE_CCW),
    ] {
        let mut mesh = grid3();
        let target = edge(&mesh, 5, 6);
        let rotated = mesh
            .rotate_edge(target, rotation, RotateChecks::empty())
            .expect("rotate");
        assert!(mesh.contains_edge(rotated));
        assert_built(&mesh, expected);
    }
}

#[test]
fn collapse_matches_blender() {
    let mut mesh = pentagon();
    let target = edge(&mesh, 1, 0);
    mesh.collapse_vert_edge(target, vert(1), true)
        .expect("collapse edge");
    assert_built(&mesh, &oracle::COLLAPSE_VERT_EDGE);

    let mut mesh = pentagon();
    let target = edge(&mesh, 1, 0);
    mesh.collapse_vert_faces(target, vert(1), 0.5, false, true)
        .expect("collapse faces");
    assert_built(&mesh, &oracle::COLLAPSE_VERT_FACES);
}

#[test]
fn triangulate_matches_blender() {
    let mut clipped = ngon();
    let result = clipped
        .triangulate_face(face(0), QuadMethod::Beauty, NgonMethod::Clip)
        .expect("clip");
    assert_eq!((result.faces.len(), result.edges.len()), (4, 4));
    assert_eq!(clipped.face_count(), oracle::TRIANGULATE_CLIP.faces.len());
    checked(&clipped);
    {
        let expected = &oracle::TRIANGULATE_BEAUTY;
        let mut mesh = ngon();
        let result = mesh
            .triangulate_face(face(0), QuadMethod::Beauty, NgonMethod::Beauty)
            .expect("triangulate");
        assert_eq!(result.faces.len(), 4);
        assert_eq!(result.edges.len(), 4);
        checked(&mesh);
        let normalized = |faces: Vec<Vec<u32>>| -> Vec<Vec<u32>> {
            let mut faces: Vec<Vec<u32>> = faces
                .into_iter()
                .map(|mut face| {
                    let start = face
                        .iter()
                        .enumerate()
                        .min_by_key(|(_, vert)| **vert)
                        .map_or(0, |(slot, _)| slot);
                    face.rotate_left(start);
                    face
                })
                .collect();
            faces.sort_unstable();
            faces
        };
        let polymesh = mesh.to_polymesh();
        let ours = normalized(
            (0..polymesh.face_count())
                .map(|face| polymesh.face_verts(face).to_vec())
                .collect(),
        );
        let theirs = normalized(expected.faces.iter().map(|face| face.to_vec()).collect());
        assert_eq!(ours, theirs);
    }
    let quads = [
        (QuadMethod::Beauty, &oracle::TRI_QUAD_BEAUTY),
        (QuadMethod::Fixed, &oracle::TRI_QUAD_FIXED),
        (QuadMethod::Alternate, &oracle::TRI_QUAD_ALTERNATE),
        (QuadMethod::ShortEdge, &oracle::TRI_QUAD_SHORT_EDGE),
        (QuadMethod::LongEdge, &oracle::TRI_QUAD_LONG_EDGE),
    ];
    for (method, expected) in quads {
        let mut mesh = quad();
        mesh.triangulate_face(face(0), method, NgonMethod::Beauty)
            .expect("triangulate quad");
        assert_built(&mesh, expected);
    }
}

#[test]
fn splice_matches_blender() {
    let mut mesh = grid3();
    mesh.splice_verts(vert(15), vert(0)).expect("splice");
    assert_built(&mesh, &oracle::SPLICE);
}

#[test]
fn collapse_edge_merges_into_target() {
    let mut mesh = grid3();
    let target = edge(&mesh, 5, 6);
    let kept = mesh.collapse_edge(target, vert(6)).expect("collapse");
    assert_eq!(kept, vert(5));
    checked(&mesh);
    assert_eq!(mesh.vert_count(), 15);
    assert_eq!(mesh.face_count(), 9);
    assert!(mesh.faces().all(|face| mesh.face_len(face) >= 3));
}

#[test]
fn non_manifold_edges_split_and_join() {
    let mut mesh = grid(2, 1);
    let fin = [mesh.add_vert(glam::Vec3::new(1.0, 0.5, 1.0))];
    let fan = mesh
        .add_face(&[vert(1), vert(4), fin[0]])
        .expect("fin face");
    let shared = edge(&mesh, 1, 4);
    assert_eq!(mesh.edge_face_count(shared), 3);
    checked(&mesh);
    let split = mesh
        .split_edge(shared, vert(1), 0.5)
        .expect("split three-face edge");
    checked(&mesh);
    assert_eq!(mesh.vert_edge_count(split.vert), 2);
    assert_eq!(mesh.face_len(fan), 4);
    let separated = mesh
        .separate_edge(
            shared,
            mesh.face_edge_share_loop(fan, shared).expect("fan loop"),
            false,
        )
        .expect("separate");
    checked(&mesh);
    assert_ne!(separated, shared);
    mesh.splice_edges(shared, separated).expect("splice back");
    checked(&mesh);
    assert_eq!(mesh.edge_face_count(shared), 3);
}

#[test]
fn dissolve_wire_chain_vert() {
    let mut mesh = EditMesh::new();
    let points: Vec<_> = (0..3)
        .map(|index| mesh.add_vert(glam::Vec3::new(index as f32, 0.0, 0.0)))
        .collect();
    mesh.add_edge(points[0], points[1]).expect("wire");
    mesh.add_edge(points[1], points[2]).expect("wire");
    mesh.dissolve_vert(points[1]).expect("dissolve wire");
    checked(&mesh);
    assert_eq!(mesh.edge_count(), 1);
    assert!(mesh.edge_between(points[0], points[2]).is_some());
}
