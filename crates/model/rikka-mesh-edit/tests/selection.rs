use glam::{Mat4, Vec3};
use rikka_mesh_edit::{
    Axes, Checker, Compare, EditMesh, Element, FaceId, IdMap, LinkedDelimit, NonManifold,
    PathOptions, SelectAction, SelectMode, SimilarFace, SimilarRegions, SimilarValue, WalkDelimit,
};
use rikka_polymesh::Polymesh;

use crate::oracle::{self, Selected};
use crate::support::{assert_selected, build, checked, edge, face, grid, grid_points, pick, vert};

fn grid65(mode: SelectMode) -> EditMesh {
    let mut mesh = grid(6, 5);
    mesh.set_select_mode(mode);
    mesh
}

fn edges(mesh: &EditMesh, pairs: &[[u32; 2]]) -> Vec<Element> {
    pairs
        .iter()
        .map(|&[a, b]| Element::Edge(edge(mesh, a, b)))
        .collect()
}

fn verts(ids: &[u32]) -> Vec<Element> {
    ids.iter()
        .map(|&index| Element::Vert(vert(index)))
        .collect()
}

fn faces(ids: &[u32]) -> Vec<Element> {
    ids.iter()
        .map(|&index| Element::Face(face(index)))
        .collect()
}

fn two_islands(wires: &[[u32; 2]]) -> EditMesh {
    let (mut points, mut polygons) = grid_points(2, 2, [0.0, 0.0]);
    let (second, second_faces) = grid_points(2, 1, [5.0, 0.0]);
    let base = points.len() as u32;
    points.extend(second);
    polygons.extend(
        second_faces
            .into_iter()
            .map(|face| face.into_iter().map(|index| index + base).collect()),
    );
    let polygons: Vec<&[u32]> = polygons.iter().map(Vec::as_slice).collect();
    build(&points, &polygons, wires)
}

#[test]
fn edge_loops_and_rings_match_blender() {
    let cases: [(u32, u32, bool, &Selected); 3] = [
        (8, 15, false, &oracle::LOOP_INTERIOR),
        (1, 2, false, &oracle::LOOP_BOUNDARY),
        (8, 15, true, &oracle::RING),
    ];
    for (a, b, ring, expected) in cases {
        let mut mesh = grid65(SelectMode::EDGE);
        let chosen = edges(&mesh, &[[a, b]]);
        pick(&mut mesh, &chosen);
        match ring {
            false => {
                mesh.select_edge_loops(WalkDelimit::LOOP_OUTER_CORNERS | WalkDelimit::LOOP_NGONS)
            }
            true => mesh.select_edge_rings(WalkDelimit::RING_NGONS),
        };
        assert_selected(&mesh, expected);
    }
}

#[test]
fn more_and_less_match_blender() {
    let cases: [(SelectMode, Vec<Element>, bool, bool, &Selected); 6] = [
        (
            SelectMode::VERT,
            verts(&[16]),
            true,
            false,
            &oracle::MORE_VERT,
        ),
        (
            SelectMode::VERT,
            verts(&[16]),
            true,
            true,
            &oracle::MORE_VERT_STEP,
        ),
        (
            SelectMode::FACE,
            faces(&[14]),
            true,
            false,
            &oracle::MORE_FACE,
        ),
        (
            SelectMode::FACE,
            faces(&[14]),
            true,
            true,
            &oracle::MORE_FACE_STEP,
        ),
        (
            SelectMode::FACE,
            faces(&[7, 8, 9, 13, 14, 15, 19, 20, 21]),
            false,
            true,
            &oracle::LESS_FACE,
        ),
        (
            SelectMode::VERT,
            verts(&[8, 9, 10, 15, 16, 17, 22, 23, 24]),
            false,
            false,
            &oracle::LESS_VERT,
        ),
    ];
    for (mode, chosen, more, face_step, expected) in cases {
        let mut mesh = grid65(mode);
        pick(&mut mesh, &chosen);
        match more {
            true => mesh.select_more(face_step),
            false => mesh.select_less(face_step),
        }
        assert_selected(&mesh, expected);
    }
}

#[test]
fn shortest_paths_match_blender() {
    let options = |topology_distance, face_step, fill| PathOptions {
        topology_distance,
        face_step,
        fill,
        ..PathOptions::default()
    };
    let cases: [(SelectMode, Vec<Element>, PathOptions, &Selected); 7] = [
        (
            SelectMode::VERT,
            verts(&[0, 33]),
            options(false, false, false),
            &oracle::PATH_VERT,
        ),
        (
            SelectMode::VERT,
            verts(&[0, 33]),
            options(true, false, false),
            &oracle::PATH_VERT_TOPO,
        ),
        (
            SelectMode::VERT,
            verts(&[0, 33]),
            options(false, true, false),
            &oracle::PATH_VERT_STEP,
        ),
        (
            SelectMode::FACE,
            faces(&[0, 29]),
            options(false, false, false),
            &oracle::PATH_FACE,
        ),
        (
            SelectMode::FACE,
            faces(&[7, 22]),
            options(false, false, true),
            &oracle::PATH_FACE_FILL,
        ),
        (
            SelectMode::VERT,
            verts(&[8, 26]),
            options(false, false, true),
            &oracle::PATH_VERT_FILL,
        ),
        (
            SelectMode::EDGE,
            Vec::new(),
            options(false, false, false),
            &oracle::PATH_EDGE,
        ),
    ];
    for (mode, chosen, options, expected) in cases {
        let mut mesh = grid65(mode);
        let chosen = match chosen.is_empty() {
            true => edges(&mesh, &[[0, 1], [33, 34]]),
            false => chosen,
        };
        pick(&mut mesh, &chosen);
        assert!(mesh.select_shortest_path(chosen[0], &options));
        assert_selected(&mesh, expected);
    }
}

#[test]
fn checker_deselect_matches_blender() {
    let row: Vec<u32> = (7..=13).collect();
    let all: Vec<u32> = (0..30).collect();
    let cases: [(SelectMode, Vec<Element>, Checker, &Selected); 3] = [
        (
            SelectMode::VERT,
            verts(&row),
            Checker {
                deselected: 1,
                selected: 1,
                offset: 0,
            },
            &oracle::NTH_ROW,
        ),
        (
            SelectMode::VERT,
            verts(&row),
            Checker {
                deselected: 2,
                selected: 1,
                offset: 1,
            },
            &oracle::NTH_ROW2,
        ),
        (
            SelectMode::FACE,
            faces(&all),
            Checker::default(),
            &oracle::NTH_FACES,
        ),
    ];
    for (mode, chosen, checker, expected) in cases {
        let mut mesh = grid65(mode);
        pick(&mut mesh, &chosen);
        assert!(mesh.deselect_checker(checker));
        assert_selected(&mesh, expected);
    }
}

#[test]
fn region_conversions_match_blender() {
    let mut mesh = grid65(SelectMode::EDGE);
    let ring = edges(
        &mesh,
        &[
            [8, 9],
            [9, 10],
            [10, 17],
            [17, 24],
            [24, 23],
            [23, 22],
            [22, 15],
            [15, 8],
        ],
    );
    pick(&mut mesh, &ring);
    mesh.select_loop_inner_region(false);
    assert_selected(&mesh, &oracle::LOOP_TO_REGION);

    let mut mesh = grid65(SelectMode::FACE);
    pick(&mut mesh, &faces(&[7, 8, 13, 14]));
    mesh.select_region_boundary();
    assert_selected(&mesh, &oracle::REGION_TO_LOOP);
}

#[test]
fn trait_selections_match_blender() {
    let mut mesh = grid65(SelectMode::FACE);
    pick(&mut mesh, &faces(&[0]));
    mesh.select_faces_by_sides(4, Compare::Equal, true);
    assert_selected(&mesh, &oracle::SIDES);

    let mut mesh = two_islands(&[]);
    pick(&mut mesh, &verts(&[0]));
    mesh.select_linked(LinkedDelimit::empty());
    assert_selected(&mesh, &oracle::LINKED);

    let mut mesh = two_islands(&[[0, 9]]);
    mesh.select_non_manifold(NonManifold::default());
    assert_selected(&mesh, &oracle::NONMANIFOLD);

    let mut mesh = two_islands(&[[0, 9]]);
    mesh.set_select_mode(SelectMode::EDGE);
    mesh.select_loose(true);
    assert_selected(&mesh, &oracle::LOOSE);
}

#[test]
fn similar_faces_match_blender() {
    let (mut points, mut polygons) = grid_points(3, 1, [0.0, 0.0]);
    points.extend([
        [0.0, 3.0, 0.0],
        [2.0, 3.0, 0.0],
        [2.0, 5.0, 0.0],
        [0.0, 5.0, 0.0],
    ]);
    polygons.push(vec![8, 9, 10, 11]);
    let polygons: Vec<&[u32]> = polygons.iter().map(Vec::as_slice).collect();
    for (kind, compare, expected) in [
        (SimilarFace::Area, Compare::Equal, &oracle::SIMILAR_AREA),
        (
            SimilarFace::Perimeter,
            Compare::Greater,
            &oracle::SIMILAR_PERIMETER_GT,
        ),
    ] {
        let mut mesh = build(&points, &polygons, &[]);
        mesh.set_select_mode(SelectMode::FACE);
        pick(&mut mesh, &faces(&[0]));
        mesh.select_similar_faces(kind, compare, 0.0);
        assert_selected(&mesh, expected);
    }
}

#[test]
fn mirror_matches_blender() {
    let (points, polygons) = grid_points(4, 2, [-2.0, -1.0]);
    let polygons: Vec<&[u32]> = polygons.iter().map(Vec::as_slice).collect();
    let mut mesh = build(&points, &polygons, &[]);
    pick(&mut mesh, &verts(&[0, 6]));
    assert_eq!(mesh.select_mirror(Axes::X, false), (2, 0));
    assert_selected(&mesh, &oracle::MIRROR);
}

#[test]
fn face_mode_switch_keeps_full_faces() {
    let mut mesh = grid65(SelectMode::VERT);
    pick(&mut mesh, &verts(&[8, 9, 15, 16, 17]));
    mesh.set_select_mode(SelectMode::FACE);
    let selected: Vec<_> = mesh
        .faces()
        .filter(|&face| mesh.is_face_selected(face))
        .collect();
    assert_eq!(selected, vec![face(7)]);
    assert!(!mesh.is_vert_selected(vert(17)));
    mesh.change_select_mode(SelectMode::VERT, false);
    mesh.change_select_mode(SelectMode::EDGE, true);
    assert!(mesh.is_edge_selected(edge(&mesh, 8, 9)));
}

#[test]
fn hide_and_reveal_flush() {
    let mut mesh = grid65(SelectMode::FACE);
    pick(&mut mesh, &faces(&[0]));
    assert!(mesh.hide_selected(false));
    assert!(mesh.is_face_hidden(face(0)));
    assert!(mesh.is_vert_hidden(vert(0)));
    assert!(!mesh.is_vert_hidden(vert(1)));
    assert!(mesh.reveal(true));
    assert!(mesh.is_face_selected(face(0)));
    checked(&mesh);
}

#[test]
fn next_active_continues_row() {
    let mut mesh = grid65(SelectMode::VERT);
    pick(&mut mesh, &verts(&[7, 8]));
    assert!(mesh.select_next_active());
    assert_eq!(mesh.active_element(), Some(Element::Vert(vert(9))));
    assert!(mesh.select_previous_active());
    assert_eq!(mesh.active_element(), Some(Element::Vert(vert(8))));
}

#[test]
fn walkers_skip_hidden_edges() {
    let mut mesh = grid65(SelectMode::EDGE);
    let blocked = edge(&mesh, 22, 29);
    mesh.hide_edge(blocked, true);
    let loop_edges = mesh.walk_edge_loop(edge(&mesh, 8, 15), &rikka_mesh_edit::Walk::visible());
    assert!(!loop_edges.contains(&blocked));
    assert_eq!(loop_edges.len(), 3);
}

fn region_mesh(case: &oracle::RegionCase) -> EditMesh {
    let mut offsets = vec![0u32];
    let mut corners = Vec::new();
    for face in case.faces {
        corners.extend_from_slice(face);
        offsets.push(corners.len() as u32);
    }
    let polymesh = Polymesh {
        positions: case
            .positions
            .iter()
            .map(|&[x, y]| Vec3::new(x as f32, y as f32, 0.0))
            .collect(),
        edges: case.edges.to_vec(),
        face_offsets: offsets,
        corner_verts: corners,
        corner_edges: case.corner_edges.to_vec(),
        ..Polymesh::default()
    };
    let mut mesh = EditMesh::from_polymesh(&polymesh).expect("region fixture");
    mesh.set_select_mode(SelectMode::FACE);
    for &index in case.picked {
        mesh.select_face(face(index), true);
    }
    mesh
}

#[test]
fn similar_region_matches_blender() {
    for case in [
        &oracle::REGION_BLOCK,
        &oracle::REGION_CORNER,
        &oracle::REGION_TRIANGLES,
    ] {
        let mut mesh = region_mesh(case);
        assert!(matches!(
            mesh.select_similar_region(),
            SimilarRegions::Found(_)
        ));
        checked(&mesh);
        let selected: Vec<u32> = mesh
            .faces()
            .filter(|&face| mesh.is_face_selected(face))
            .map(FaceId::raw)
            .collect();
        assert_eq!(selected, case.selected);
    }
}

#[test]
fn similar_region_needs_two_faces() {
    let mut mesh = region_mesh(&oracle::REGION_CORNER);
    mesh.select_all(SelectAction::Deselect);
    mesh.select_face(face(0), true);
    assert_eq!(mesh.select_similar_region(), SimilarRegions::TooFewFaces);
}

#[test]
fn similar_faces_compare_in_world_space() {
    let mut reference = build(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        &[&[0, 1, 2, 3]],
        &[],
    );
    reference.select_face(face(0), true);
    let scale = Mat4::from_scale(Vec3::splat(2.0));
    let values = reference.selected_similar_face_values(SimilarFace::Area, &scale);
    assert_eq!(values, [SimilarValue::Scalar(4.0)]);
    let mut target = build(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [3.0, 0.0, 0.0],
            [5.0, 0.0, 0.0],
            [5.0, 2.0, 0.0],
            [3.0, 2.0, 0.0],
        ],
        &[&[0, 1, 2, 3], &[4, 5, 6, 7]],
        &[],
    );
    target.set_select_mode(SelectMode::FACE);
    let changed = target.select_similar_faces_matching(
        SimilarFace::Area,
        Compare::Equal,
        0.0,
        &values,
        &Mat4::IDENTITY,
    );
    assert!(changed);
    assert!(!target.is_face_selected(face(0)));
    assert!(target.is_face_selected(face(1)));
}

#[test]
fn mirror_selection_follows_given_map() {
    let mut mesh = grid65(SelectMode::VERT);
    pick(&mut mesh, &verts(&[0]));
    let mut map = IdMap::new();
    map.insert(vert(0), vert(5));
    let (mirrored, failed) = mesh.select_mirror_mapped(Axes::X, false, Some(&map));
    assert_eq!((mirrored, failed), (1, 0));
    let selected: Vec<u32> = mesh
        .verts()
        .filter(|&v| mesh.is_vert_selected(v))
        .map(rikka_mesh_edit::VertId::raw)
        .collect();
    assert_eq!(selected, [5]);
}
