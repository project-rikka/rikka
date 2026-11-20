use glam::{Vec2, Vec3};
use rikka_mesh_edit::{DataType, Domain, EditMesh, Flags, LoopId};

use crate::support::{TOLERANCE, checked, corner, edge, face, grid, vert};

fn textured_quad() -> EditMesh {
    let mut mesh = grid(1, 1);
    mesh.add_uv_map("UVMap").expect("uv map");
    let loops: Vec<LoopId> = mesh.face_loops(face(0)).collect();
    let uvs = mesh.loop_uvs_mut("UVMap").expect("uvs");
    for (lp, uv) in loops.iter().zip([
        Vec2::new(0.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(0.0, 1.0),
    ]) {
        uvs[lp.index()] = uv;
    }
    mesh
}

#[test]
fn split_edge_interpolates_all_data() {
    let mut mesh = textured_quad();
    mesh.add_attribute("heat", Domain::Point, DataType::Float)
        .expect("attribute");
    mesh.attribute_mut::<f32>("heat", Domain::Point)
        .expect("heat")[1] = 4.0;
    let group = mesh.add_vertex_group("Bone");
    mesh.set_vert_weight(vert(1), group, 1.0);
    let target = edge(&mesh, 0, 1);
    mesh.edge_creases_mut()[target.index()] = 0.5;
    mesh.set_edge_seam(target, true);
    let split = mesh.split_edge(target, vert(0), 0.25).expect("split");
    checked(&mesh);
    assert_eq!(mesh.position(split.vert), Vec3::new(0.25, 0.0, 0.0));
    assert_eq!(
        mesh.attribute::<f32>("heat", Domain::Point).expect("heat")[split.vert.index()],
        1.0
    );
    assert!((mesh.vert_weight(split.vert, group) - 0.25).abs() < TOLERANCE);
    assert_eq!(
        mesh.edge_creases().expect("creases")[split.edge.index()],
        0.5
    );
    assert!(mesh.is_edge_seam(split.edge));
    let new_corner = mesh
        .face_vert_share_loop(face(0), split.vert)
        .expect("corner");
    let uv = mesh.loop_uvs("UVMap").expect("uvs")[new_corner.index()];
    assert!(uv.distance(Vec2::new(0.25, 0.0)) < TOLERANCE);
}

#[test]
fn loop_interpolation_uses_mean_value_weights() {
    let mut mesh = textured_quad();
    let sample = mesh.sample_face(face(0));
    let center = mesh.add_vert(Vec3::new(0.5, 0.25, 0.0));
    let triangle = mesh
        .add_face(&[vert(0), vert(1), center])
        .expect("triangle");
    let lp = mesh
        .face_vert_share_loop(triangle, center)
        .expect("center corner");
    mesh.interpolate_loop_from_sample(lp, &sample, false);
    let uv = mesh.loop_uvs("UVMap").expect("uvs")[lp.index()];
    assert!(uv.distance(Vec2::new(0.5, 0.25)) < TOLERANCE);
    mesh.interpolate_face_from_face(triangle, face(0), false);
    let first = mesh
        .face_vert_share_loop(triangle, vert(1))
        .expect("corner");
    assert!(
        mesh.loop_uvs("UVMap").expect("uvs")[first.index()].distance(Vec2::new(1.0, 0.0))
            < TOLERANCE
    );
    checked(&mesh);
}

#[test]
fn averaging_and_copy_keep_selection_of_target() {
    let mut mesh = grid(2, 1);
    mesh.select_vert(vert(0), true);
    mesh.set_vert_flag(vert(0), Flags::HIDDEN, true);
    mesh.copy_vert_data(vert(0), vert(5));
    assert!(mesh.is_vert_hidden(vert(5)));
    assert!(!mesh.is_vert_selected(vert(5)));
    mesh.add_attribute("w", Domain::Point, DataType::Float)
        .expect("attribute");
    let values = mesh.attribute_mut::<f32>("w", Domain::Point).expect("w");
    values[1] = 1.0;
    values[2] = 3.0;
    mesh.average_vert_data(&[vert(1), vert(2)], vert(3));
    assert_eq!(
        mesh.attribute::<f32>("w", Domain::Point).expect("w")[3],
        2.0
    );
}

#[test]
fn uv_contiguity_follows_shared_corners() {
    let mut mesh = grid(2, 1);
    mesh.add_uv_map("UVMap").expect("uv map");
    let shared = edge(&mesh, 1, 4);
    assert!(mesh.is_edge_uv_contiguous(shared, "UVMap"));
    let lp = corner(&mesh, 1, 1);
    mesh.loop_uvs_mut("UVMap").expect("uvs")[lp.index()] = Vec2::new(5.0, 5.0);
    assert!(!mesh.is_edge_uv_contiguous(shared, "UVMap"));
}

#[test]
fn reserved_attribute_names_are_rejected() {
    let mut mesh = grid(1, 1);
    assert!(
        mesh.add_attribute("material_index", Domain::Face, DataType::Int)
            .is_err()
    );
    assert!(
        mesh.add_attribute("x", Domain::Edge, DataType::Bool)
            .is_ok()
    );
    assert!(
        mesh.add_attribute("x", Domain::Face, DataType::Bool)
            .is_err()
    );
    assert!(mesh.remove_attribute("x"));
}
