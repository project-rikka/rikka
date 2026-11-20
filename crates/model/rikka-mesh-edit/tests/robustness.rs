use glam::Mat4;
use rikka_mesh_edit::{
    Axes, Checker, Compare, EdgePathTag, EditMesh, Element, LinkedDelimit, NonManifold,
    PathOptions, PickAction, SelectAction, SelectMode, SimilarEdge, SimilarFace, SimilarVert, Walk,
    WalkDelimit, topology_mirror_table,
};
use rikka_test_meshes::{
    EditOperator, Rng, corpus, random_edge, random_face, random_vert, sample_fixtures,
    sweep_chains, sweep_edit,
};

const CASES: usize = 24;
const SAMPLE_CASES: usize = 8;
const CHAINS: usize = 24;
const CHAIN_LENGTH: usize = 10;
const COMPARES: [Compare; 4] = [
    Compare::Equal,
    Compare::NotEqual,
    Compare::Greater,
    Compare::Less,
];

fn element(mesh: &EditMesh, rng: &mut Rng) -> Option<Element> {
    match rng.below(3) {
        0 => random_vert(mesh, rng).map(Element::Vert),
        1 => random_edge(mesh, rng).map(Element::Edge),
        _ => random_face(mesh, rng).map(Element::Face),
    }
}

fn walk_delimit(rng: &mut Rng) -> WalkDelimit {
    WalkDelimit::from_bits_truncate(rng.below(128) as u8)
}

fn linked_delimit(rng: &mut Rng) -> LinkedDelimit {
    LinkedDelimit::from_bits_truncate(rng.below(32) as u8)
}

fn mode(rng: &mut Rng) -> SelectMode {
    SelectMode::from_bits_truncate(rng.int(1, 7) as u8)
}

fn operators() -> Vec<(&'static str, EditOperator)> {
    vec![
        ("select_all", |mesh, rng| {
            mesh.select_all(rng.pick(&[
                SelectAction::Select,
                SelectAction::Deselect,
                SelectAction::Invert,
                SelectAction::Toggle,
            ]));
            Ok(())
        }),
        ("select_more_less", |mesh, rng| {
            match rng.flip() {
                true => mesh.select_more(rng.flip()),
                false => mesh.select_less(rng.flip()),
            }
            Ok(())
        }),
        ("select_linked", |mesh, rng| {
            mesh.select_linked(linked_delimit(rng));
            Ok(())
        }),
        ("select_linked_pick", |mesh, rng| {
            if let Some(element) = element(mesh, rng) {
                mesh.select_linked_pick(element, rng.flip(), linked_delimit(rng));
            }
            Ok(())
        }),
        ("pick_loop", |mesh, rng| {
            if let Some(edge) = random_edge(mesh, rng) {
                let action = rng.pick(&[
                    PickAction::Set,
                    PickAction::Extend,
                    PickAction::Deselect,
                    PickAction::Toggle,
                ]);
                mesh.pick_loop(edge, rng.flip(), action, walk_delimit(rng));
            }
            Ok(())
        }),
        ("select_edge_loops", |mesh, rng| {
            mesh.select_edge_loops(walk_delimit(rng));
            Ok(())
        }),
        ("select_edge_rings", |mesh, rng| {
            mesh.select_edge_rings(walk_delimit(rng));
            Ok(())
        }),
        ("select_boundary_loops", |mesh, rng| {
            mesh.select_boundary_loops(rng.flip(), walk_delimit(rng));
            Ok(())
        }),
        ("select_region_boundary", |mesh, _| {
            mesh.select_region_boundary();
            Ok(())
        }),
        ("select_loop_inner_region", |mesh, rng| {
            mesh.select_loop_inner_region(rng.flip());
            Ok(())
        }),
        ("select_shortest_path", |mesh, rng| {
            let Some(to) = element(mesh, rng) else {
                return Ok(());
            };
            let options = PathOptions {
                topology_distance: rng.flip(),
                face_step: rng.flip(),
                fill: rng.flip(),
                edge_tag: rng.pick(&[
                    EdgePathTag::Select,
                    EdgePathTag::Seam,
                    EdgePathTag::Sharp,
                    EdgePathTag::Crease,
                    EdgePathTag::BevelWeight,
                ]),
                interval: rng.flip().then(|| Checker {
                    deselected: rng.int(0, 3),
                    selected: rng.int(0, 3),
                    offset: rng.int(0, 6) as i32 - 3,
                }),
            };
            if let Some(from) = element(mesh, rng)
                && std::mem::discriminant(&from) == std::mem::discriminant(&to)
            {
                drop(mesh.shortest_path(from, to, &options));
                drop(mesh.path_region(from, to));
            }
            mesh.select_shortest_path(to, &options);
            Ok(())
        }),
        ("select_similar", |mesh, rng| {
            let compare = rng.pick(&COMPARES);
            let threshold = rng.range(0.0, 1.0);
            match rng.below(4) {
                0 => {
                    let kind = rng.pick(&[
                        SimilarVert::Normal,
                        SimilarVert::Faces,
                        SimilarVert::Groups,
                        SimilarVert::Edges,
                        SimilarVert::Crease,
                    ]);
                    mesh.select_similar_verts(kind, compare, threshold);
                }
                1 => {
                    let kind = rng.pick(&[
                        SimilarEdge::Length,
                        SimilarEdge::Direction,
                        SimilarEdge::Faces,
                        SimilarEdge::FaceAngle,
                        SimilarEdge::Crease,
                        SimilarEdge::Bevel,
                        SimilarEdge::Seam,
                        SimilarEdge::Sharp,
                        SimilarEdge::Freestyle,
                    ]);
                    mesh.select_similar_edges(kind, compare, threshold);
                }
                2 => {
                    let kind = rng.pick(&[
                        SimilarFace::Material,
                        SimilarFace::Area,
                        SimilarFace::Sides,
                        SimilarFace::Perimeter,
                        SimilarFace::Normal,
                        SimilarFace::Coplanar,
                        SimilarFace::Smooth,
                        SimilarFace::Freestyle,
                    ]);
                    mesh.select_similar_faces(kind, compare, threshold);
                }
                _ => {
                    let _ = mesh.select_similar_region();
                }
            }
            Ok(())
        }),
        ("select_random", |mesh, rng| {
            mesh.select_random(rng.unit(), rng.int(0, 1000), rng.flip());
            Ok(())
        }),
        ("deselect_checker", |mesh, rng| {
            mesh.deselect_checker(Checker {
                deselected: rng.int(0, 3),
                selected: rng.int(0, 3),
                offset: rng.int(0, 8) as i32 - 4,
            });
            Ok(())
        }),
        ("select_non_manifold", |mesh, rng| {
            mesh.select_non_manifold(NonManifold {
                extend: rng.flip(),
                wire: rng.flip(),
                boundary: rng.flip(),
                multi_face: rng.flip(),
                non_contiguous: rng.flip(),
                verts: rng.flip(),
            });
            Ok(())
        }),
        ("select_loose", |mesh, rng| {
            mesh.select_loose(rng.flip());
            Ok(())
        }),
        ("select_interior_faces", |mesh, _| {
            mesh.select_interior_faces();
            Ok(())
        }),
        ("select_faces_by_sides", |mesh, rng| {
            mesh.select_faces_by_sides(rng.below(8), rng.pick(&COMPARES), rng.flip());
            Ok(())
        }),
        ("select_sharp_edges", |mesh, rng| {
            mesh.select_sharp_edges(rng.range(0.0, std::f32::consts::PI));
            Ok(())
        }),
        ("select_linked_flat_faces", |mesh, rng| {
            mesh.select_linked_flat_faces(rng.range(0.0, std::f32::consts::PI));
            Ok(())
        }),
        ("select_mirror", |mesh, rng| {
            let axes = Axes::from_bits_truncate(rng.below(8) as u8);
            match rng.flip() {
                true => {
                    let _ = mesh.select_mirror(axes, rng.flip());
                }
                false => {
                    let verts: Vec<_> = mesh.verts().collect();
                    let index = |vert| verts.iter().position(|&other| other == vert);
                    let edges: Vec<[u32; 2]> = mesh
                        .edges()
                        .filter_map(|edge| {
                            let [a, b] = mesh.edge_verts(edge);
                            Some([index(a)? as u32, index(b)? as u32])
                        })
                        .collect();
                    let table = topology_mirror_table(verts.len(), &edges);
                    let mut map = rikka_mesh_edit::IdMap::new();
                    for (vert, mirror) in verts.iter().zip(&table) {
                        if let Some(mirror) = mirror {
                            map.insert(*vert, verts[*mirror as usize]);
                        }
                    }
                    let _ = mesh.select_mirror_mapped(axes, rng.flip(), Some(&map));
                }
            }
            Ok(())
        }),
        ("select_by_material", |mesh, rng| {
            mesh.select_by_material(rng.int(0, 3) as i32, rng.flip());
            Ok(())
        }),
        ("select_verts_where", |mesh, rng| {
            let limit = rng.range(-1.0, 1.0);
            mesh.select_verts_where(rng.flip(), |mesh, vert| mesh.position(vert).x > limit);
            Ok(())
        }),
        ("select_vertex_group", |mesh, rng| {
            let group = rng.below(mesh.vertex_groups().len() + 1) as u32;
            mesh.select_vertex_group(group, rng.flip());
            mesh.select_ungrouped(rng.flip());
            Ok(())
        }),
        ("select_previous_active", |mesh, _| {
            mesh.select_previous_active();
            Ok(())
        }),
        ("change_select_mode", |mesh, rng| {
            let mode = mode(rng);
            match rng.flip() {
                true => mesh.change_select_mode(mode, rng.flip()),
                false => mesh.set_select_mode(mode),
            }
            Ok(())
        }),
        ("hide_reveal", |mesh, rng| {
            match rng.flip() {
                true => {
                    mesh.hide_selected(rng.flip());
                }
                false => {
                    mesh.reveal(rng.flip());
                }
            }
            Ok(())
        }),
        ("walkers", |mesh, rng| {
            let walk = Walk::visible().delimit(walk_delimit(rng));
            if let Some(edge) = random_edge(mesh, rng) {
                drop(mesh.walk_edge_loop(edge, &walk));
                drop(mesh.walk_face_loop(edge, &walk));
                drop(mesh.walk_edge_ring(edge, &walk));
                drop(mesh.walk_boundary(edge, &walk));
                drop(mesh.walk_non_manifold_loop(edge));
            }
            if let Some(face) = random_face(mesh, rng) {
                drop(mesh.walk_island(face, &walk, rng.flip()));
                drop(mesh.walk_island_delimited(face, linked_delimit(rng)));
            }
            if let Some(vert) = random_vert(mesh, rng) {
                drop(mesh.walk_connected_verts(vert, &walk));
            }
            drop(mesh.face_islands(linked_delimit(rng)));
            Ok(())
        }),
        ("polymesh_round_trip", |mesh, _| {
            let polymesh = mesh.to_polymesh();
            let restored = EditMesh::from_polymesh(&polymesh).map_err(|error| error.to_string())?;
            match restored.to_polymesh() == polymesh {
                true => Ok(()),
                false => Err("polymesh round trip changed the mesh".to_owned()),
            }
        }),
        ("compact", |mesh, _| {
            drop(mesh.compact());
            drop(mesh.tessellate());
            Ok(())
        }),
        ("duplicate_geometry", |mesh, _| {
            let geometry = rikka_test_meshes::selected_geometry(mesh);
            drop(mesh.duplicate(&geometry));
            Ok(())
        }),
        ("transform_selected", |mesh, rng| {
            let matrix = Mat4::from_axis_angle(rng.direction(), rng.range(0.0, 3.0));
            let verts: Vec<_> = rikka_test_meshes::selected_verts(mesh).iter().collect();
            for vert in verts {
                let moved = matrix.transform_point3(mesh.position(vert));
                mesh.set_position(vert, moved);
            }
            mesh.update_normals();
            Ok(())
        }),
    ]
}

#[test]
fn selection_operators_survive_corpus() {
    sweep_edit("selection corpus", &corpus(), CASES, &operators());
}

#[test]
fn selection_operators_survive_samples() {
    sweep_edit(
        "selection samples",
        &sample_fixtures(),
        SAMPLE_CASES,
        &operators(),
    );
}

#[test]
fn selection_operator_chains_survive() {
    let mut fixtures = corpus();
    fixtures.extend(sample_fixtures());
    sweep_chains(
        "selection chains",
        &fixtures,
        CHAINS,
        CHAIN_LENGTH,
        &operators(),
    );
}
