use rikka_mesh_edit::{EdgeId, EditMesh, VertId};

use crate::support::{build, checked};

#[test]
fn edgenet_path_through_coincident_vertex_splits_face() {
    let mut mesh = build(
        &[
            [0.496_310_77, 0.234_737_5, -0.835_807_4],
            [0.507_839, 0.240_190_01, -0.827_286_24],
            [0.508_117_26, 0.240_321_6, -0.827_080_55],
            [0.509_737_43, 0.236_780_8, -0.827_093_4],
            [0.509_88, 0.236_474_36, -0.827_075_06],
            [0.513_861_95, 0.227_779_4, -0.827_080_55],
            [0.501_922, 0.222_486_72, -0.835_807_4],
            [0.508_473_34, 0.239_036_25, -0.827_222],
            [0.508_733_45, 0.238_565_03, -0.827_195],
            [0.509_737_43, 0.236_780_8, -0.827_093_4],
        ],
        &[&[0, 1, 2, 3, 4, 5, 6]],
        &[[7, 1], [8, 7], [9, 8], [4, 9]],
    );
    let face = mesh.faces().next().expect("face");
    let verts: Vec<VertId> = mesh.verts().collect();
    let edge = |mesh: &EditMesh, a: usize, b: usize| -> EdgeId {
        mesh.edge_between(verts[a], verts[b]).expect("edge")
    };
    let net = [
        edge(&mesh, 7, 1),
        edge(&mesh, 8, 7),
        edge(&mesh, 9, 8),
        edge(&mesh, 4, 9),
    ];
    let created = mesh.split_face_edgenet(face, &net).expect("split");
    checked(&mesh);
    let index = |vert: VertId| verts.iter().position(|&known| known == vert).expect("vert");
    let faces: Vec<Vec<usize>> = created
        .iter()
        .map(|&face| mesh.face_verts_vec(face).into_iter().map(index).collect())
        .collect();
    assert_eq!(
        faces,
        [vec![0, 1, 7, 8, 9, 4, 5, 6], vec![4, 9, 8, 7, 1, 2, 3]]
    );
    assert_eq!(mesh.face_count(), 2);
}
