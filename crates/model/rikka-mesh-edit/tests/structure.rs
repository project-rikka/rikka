use glam::Vec3;
use rikka_mesh_edit::{DeleteContext, EditMesh, Error, Geometry, IdSet, Marks, VertId};

use crate::support::{checked, edge, face, grid, vert};

#[test]
fn create_and_cascade_kill() {
    let mut mesh = grid(2, 2);
    assert_eq!(
        (
            mesh.vert_count(),
            mesh.edge_count(),
            mesh.face_count(),
            mesh.loop_count()
        ),
        (9, 12, 4, 16)
    );
    let center = vert(4);
    assert_eq!(mesh.vert_edge_count(center), 4);
    assert_eq!(mesh.vert_face_count(center), 4);
    assert!(mesh.is_vert_manifold(center));
    mesh.kill_vert(center).expect("kill");
    checked(&mesh);
    assert_eq!(
        (mesh.vert_count(), mesh.edge_count(), mesh.face_count()),
        (8, 8, 0)
    );
    assert!(!mesh.contains_vert(center));
    let reused = mesh.add_vert(Vec3::ONE);
    assert_eq!(reused, center);
    assert_eq!(mesh.position(reused), Vec3::ONE);
}

#[test]
fn face_creation_validates_input() {
    let mut mesh = grid(1, 1);
    let corners = [vert(0), vert(1), vert(3)];
    assert!(matches!(
        mesh.add_face(&[vert(0), vert(1)]),
        Err(Error::TooFewCorners)
    ));
    assert!(matches!(
        mesh.add_face(&[vert(0), vert(0), vert(1)]),
        Err(Error::DegenerateEdge)
    ));
    assert!(matches!(
        mesh.new_face(&corners).existing_edges().create(),
        Err(Error::MissingEdge(..))
    ));
    let existing = mesh
        .new_face(&[vert(0), vert(1), vert(3), vert(2)])
        .reuse_existing()
        .create()
        .expect("reuse");
    assert_eq!(existing, face(0));
    assert_eq!(
        mesh.face_exists(&[vert(2), vert(3), vert(1), vert(0)]),
        Some(face(0))
    );
    let double = mesh
        .add_face(&[vert(0), vert(1), vert(3), vert(2)])
        .expect("double");
    assert_eq!(mesh.face_find_double(double), Some(face(0)));
    assert_eq!(mesh.edge_face_count(edge(&mesh, 0, 1)), 2);
    checked(&mesh);
    let mut dead = mesh.clone();
    dead.kill_vert(vert(0)).expect("kill");
    assert!(matches!(
        dead.add_face(&[vert(0), vert(1), vert(3)]),
        Err(Error::DeadVert(_))
    ));
}

#[test]
fn delete_contexts_follow_blender() {
    let cases = [
        (DeleteContext::Verts, (8, 8, 0)),
        (DeleteContext::Faces, (0, 0, 0)),
        (DeleteContext::OnlyFaces, (9, 12, 0)),
        (DeleteContext::EdgesFaces, (9, 8, 0)),
    ];
    for (context, counts) in cases {
        let mut mesh = grid(2, 2);
        let mut geometry = Geometry::new();
        match context {
            DeleteContext::Verts => {
                geometry.verts.insert(vert(4));
            }
            DeleteContext::EdgesFaces => {
                for pair in [[1, 4], [3, 4], [4, 5], [4, 7]] {
                    geometry.edges.insert(edge(&mesh, pair[0], pair[1]));
                }
            }
            _ => geometry.faces.extend(mesh.faces()),
        }
        mesh.delete(context, &geometry);
        checked(&mesh);
        assert_eq!(
            (mesh.vert_count(), mesh.edge_count(), mesh.face_count()),
            counts,
            "{context:?}"
        );
    }
}

#[test]
fn kill_face_loose_removes_orphans() {
    let mut mesh = grid(2, 1);
    mesh.kill_face_loose(face(0)).expect("kill");
    checked(&mesh);
    assert_eq!(
        (mesh.vert_count(), mesh.edge_count(), mesh.face_count()),
        (4, 4, 1)
    );
}

#[test]
fn duplicate_copies_region_with_maps() {
    let mut mesh = grid(2, 1);
    let mut geometry = Geometry::new();
    geometry.faces.insert(face(1));
    let copy = mesh.duplicate(&geometry);
    checked(&mesh);
    assert_eq!(copy.verts.len(), 4);
    assert_eq!(copy.edges.len(), 4);
    let new_face = copy.faces.copied(face(1)).expect("copied face");
    let originals: Vec<VertId> = mesh.face_verts(face(1)).collect();
    let copied: Vec<VertId> = mesh.face_verts(new_face).collect();
    let mapped: Vec<VertId> = originals
        .iter()
        .map(|&vert| copy.verts.copied(vert).expect("vert"))
        .collect();
    assert_eq!(copied, mapped);
    assert!(mesh.is_edge_boundary(mesh.loop_edge(mesh.face_first_loop(new_face))));
}

#[test]
fn id_sets_and_marks_track_membership() {
    let mut set: IdSet<VertId> = IdSet::new();
    assert!(set.insert(vert(70)));
    assert!(!set.insert(vert(70)));
    set.insert(vert(3));
    assert_eq!(set.to_vec(), vec![vert(3), vert(70)]);
    assert!(set.remove(vert(70)));
    assert_eq!(set.len(), 1);
    let mut marks: Marks<VertId> = Marks::new();
    marks.enable(vert(5), 0b101);
    marks.disable(vert(5), 0b001);
    assert!(marks.test(vert(5), 0b100));
    assert!(!marks.test(vert(5), 0b001));
    assert_eq!(marks.iter_with(0b100).collect::<Vec<_>>(), vec![vert(5)]);
}

#[test]
fn revisions_change_by_kind() {
    let mut mesh = grid(1, 1);
    let before = mesh.revisions();
    mesh.set_position(vert(0), Vec3::NEG_ONE);
    let shaped = mesh.revisions();
    assert_ne!(before.shape, shaped.shape);
    assert_eq!(before.topology, shaped.topology);
    mesh.select_vert(vert(0), true);
    assert_ne!(shaped.selection, mesh.revisions().selection);
    let copy = mesh.clone();
    assert_ne!(copy.revisions().mesh, mesh.revisions().mesh);
}

#[test]
fn layer_list_edits_bump_revisions() {
    let mut mesh = grid(1, 1);
    mesh.add_uv_map("First").expect("uv map");
    mesh.add_uv_map("Second").expect("uv map");
    mesh.add_color_attribute("A", rikka_mesh_edit::Domain::Point)
        .expect("color");
    mesh.add_color_attribute("B", rikka_mesh_edit::Domain::Corner)
        .expect("color");
    mesh.add_key_block(rikka_mesh_edit::KeyBlock::new("Basis"));
    type Edit = fn(&mut EditMesh);
    let edits: [(&str, Edit); 8] = [
        ("add vertex group", |mesh| {
            let _ = mesh.add_vertex_group("Group");
        }),
        ("set active vertex group", |mesh| {
            mesh.set_active_vertex_group(None)
        }),
        ("vertex groups", |mesh| {
            let _ = mesh.vertex_groups_mut();
        }),
        ("active uv map", |mesh| {
            let _ = mesh.set_active_uv_map("Second");
        }),
        ("render uv map", |mesh| {
            let _ = mesh.set_render_uv_map("Second");
        }),
        ("active color", |mesh| {
            let _ = mesh.set_active_color_attribute("B");
        }),
        ("render color", |mesh| {
            let _ = mesh.set_render_color_attribute("B");
        }),
        ("key blocks", |mesh| {
            let _ = mesh.key_blocks_mut();
        }),
    ];
    for (name, edit) in edits {
        let before = mesh.revisions();
        edit(&mut mesh);
        let after = mesh.revisions();
        assert!(
            before.data != after.data || before.keys != after.keys,
            "{name} kept the revisions"
        );
    }
}

#[test]
fn tessellation_updates_moved_faces() {
    let mut mesh = grid(2, 2);
    let mut tessellation = mesh.tessellate();
    assert_eq!(tessellation.triangles.len(), 8);
    mesh.set_position(vert(4), Vec3::new(1.0, 1.0, 1.0));
    tessellation.update(&mesh, [vert(4)]);
    assert_eq!(tessellation, mesh.tessellate());
    mesh.kill_face(face(0)).expect("kill");
    assert!(!tessellation.is_current(&mesh));
    tessellation.update(&mesh, []);
    assert_eq!(tessellation.triangles.len(), 6);
    assert!(tessellation.face_triangles(face(0)).is_empty());
}

#[test]
fn normals_follow_blender_weighting() {
    let mut mesh = grid(1, 1);
    mesh.update_normals();
    assert_eq!(mesh.face_normal(face(0)), Vec3::Z);
    assert_eq!(mesh.vert_normal(vert(0)), Vec3::Z);
    let lonely = mesh.add_vert(Vec3::new(0.0, 3.0, 4.0));
    mesh.update_normals_around([lonely]);
    assert_eq!(mesh.vert_normal(lonely), Vec3::new(0.0, 0.6, 0.8));
    assert!((mesh.face_area(face(0)) - 1.0).abs() < 1.0e-6);
    assert_eq!(mesh.face_center_median(face(0)), Vec3::new(0.5, 0.5, 0.0));
}

#[test]
fn split_face_n_inserts_path_verts() {
    let mut mesh = grid(1, 1);
    let a = mesh.face_vert_share_loop(face(0), vert(0)).expect("corner");
    let b = mesh.face_vert_share_loop(face(0), vert(3)).expect("corner");
    let split = mesh
        .split_face_n(
            face(0),
            a,
            b,
            &[Vec3::new(0.4, 0.4, 0.0), Vec3::new(0.6, 0.6, 0.0)],
            None,
        )
        .expect("split");
    checked(&mesh);
    assert_eq!(split.verts.len(), 2);
    assert_eq!(mesh.face_count(), 2);
    assert_eq!(mesh.face_len(face(0)) + mesh.face_len(split.face), 10);
    let connected = mesh.connect_verts(vert(1), split.verts[0]);
    assert!(connected.is_ok());
    checked(&mesh);
}
