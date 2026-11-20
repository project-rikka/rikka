use rikka_polymesh::{NgonMethod, Polymesh, QuadMethod};

use crate::support::{cube, points};

type Quad = [[f32; 3]; 4];
type Split = [[u32; 3]; 2];

#[test]
fn concave_ngon_matches_blender_tessellation() {
    let mesh = Polymesh::from_polygons(
        points(&[
            [0.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
            [4.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 3.0, 0.0],
            [4.0, 3.0, 0.0],
            [4.0, 4.0, 0.0],
            [0.0, 4.0, 0.0],
        ]),
        [[0, 1, 2, 3, 4, 5, 6, 7]],
    );
    let triangulation = mesh.triangulate();
    assert_eq!(
        triangulation.triangles,
        vec![
            [0, 1, 2],
            [0, 2, 3],
            [5, 6, 7],
            [0, 3, 4],
            [4, 5, 7],
            [0, 4, 7]
        ]
    );
    assert_eq!(triangulation.faces, vec![0; 6]);
}

#[test]
fn quads_split_like_blender() {
    let cases: [(Quad, Split); 3] = [
        (
            [
                [0.0, 0.0, 0.0],
                [4.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 4.0, 0.0],
            ],
            [[0, 1, 2], [0, 2, 3]],
        ),
        (
            [
                [0.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [4.0, 0.0, 0.0],
                [1.0, 4.0, 0.0],
            ],
            [[0, 1, 3], [1, 2, 3]],
        ),
        (
            [
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.7],
                [2.0, 2.0, 0.0],
                [0.0, 2.0, 0.9],
            ],
            [[0, 1, 2], [0, 2, 3]],
        ),
    ];
    for (quad, expected) in cases {
        let mesh = Polymesh::from_polygons(points(&quad), [[0, 1, 2, 3]]);
        assert_eq!(mesh.triangulate().triangles, expected.to_vec(), "{quad:?}");
    }
}

#[test]
fn whole_mesh_triangulation_names_owning_faces() {
    let triangulation = cube().triangulate();
    assert_eq!(triangulation.triangles.len(), 12);
    assert_eq!(
        triangulation.faces,
        vec![0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5]
    );
}

#[test]
fn quad_methods_choose_diagonals() {
    let mesh = Polymesh::from_polygons(
        points(&[
            [0.0, 0.0, 0.0],
            [3.0, 0.0, 0.0],
            [3.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ]),
        [[0, 1, 2, 3]],
    );
    let along_02 = vec![[0, 1, 2], [0, 2, 3]];
    let along_13 = vec![[0, 1, 3], [1, 2, 3]];
    for (method, expected) in [
        (QuadMethod::Fixed, &along_02),
        (QuadMethod::Alternate, &along_13),
        (QuadMethod::ShortEdge, &along_13),
        (QuadMethod::LongEdge, &along_02),
    ] {
        assert_eq!(
            &mesh.triangulate_face_with(0, method, NgonMethod::Beauty),
            expected,
            "{method:?}"
        );
    }
}
