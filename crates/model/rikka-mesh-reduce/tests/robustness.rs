use glam::Vec3;
use rikka_mesh_edit::{EditMesh, IdMap, IdSet, SelectAction, SelectMode, VertId};
use rikka_mesh_reduce::{
    AutoMergeOptions, Axis, Collapse, DecimateOptions, DeleteEdgeLoopOptions, DeleteLooseOptions,
    DeleteOptions, DeleteType, Delimit, DissolveDegenerateOptions, DissolveEdgesOptions,
    DissolveFacesOptions, DissolveModeOptions, DissolveVertsOptions, FindDoublesOptions,
    JoinTrianglesOptions, JoinTrianglesSettings, LimitedDissolveOptions, MergeOptions, MergeType,
    RemoveDoublesOptions, UnsubdivideOptions, WeldOptions, auto_merge, auto_merge_split,
    average_face_data, collapse_edges, collapse_geometry, collapse_short_edges, decimate,
    decimate_collapse, decimate_planar, decimate_unsubdivide, delete, delete_edge_loop,
    delete_loose, dissolve_degenerate, dissolve_degenerate_geometry, dissolve_edges,
    dissolve_edges_geometry, dissolve_faces, dissolve_faces_geometry, dissolve_limited,
    dissolve_limited_geometry, dissolve_mode, dissolve_verts, dissolve_verts_geometry,
    find_doubles, intersect_edges, join_triangles, join_triangles_geometry, merge,
    merge_face_data_at, point_merge, remove_doubles, remove_duplicate_faces, unsubdivide,
    unsubdivide_geometry, weld,
};
use rikka_test_meshes::{
    EditOperator, Rng, bounds, corpus, outcome, random_vert, sample_fixtures, selected_edges,
    selected_faces, selected_verts, sweep_chains, sweep_edit,
};

use crate::support::checked;

const CASES: usize = 24;
const SAMPLE_CASES: usize = 8;
const CHAINS: usize = 32;
const CHAIN_LENGTH: usize = 10;
const DELETE_TYPES: [DeleteType; 5] = [
    DeleteType::Verts,
    DeleteType::Edges,
    DeleteType::Faces,
    DeleteType::EdgesFaces,
    DeleteType::OnlyFaces,
];
const MERGE_TYPES: [MergeType; 5] = [
    MergeType::Center,
    MergeType::Cursor,
    MergeType::Collapse,
    MergeType::First,
    MergeType::Last,
];

fn messy() -> EditMesh {
    let mut mesh = EditMesh::new();
    let points = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.5, 0.5, 1.0],
        [0.5, 0.5, -1.0],
        [2.0, 0.0, 0.0],
        [3.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [5.0, 5.0, 5.0],
        [1.0, 0.0, 0.0],
        [4.0, 1.0, 0.0],
        [4.5, 1.0, 0.0],
    ];
    let verts: Vec<VertId> = points
        .iter()
        .map(|&point| mesh.add_vert(Vec3::from_array(point)))
        .collect();
    let faces: [&[usize]; 8] = [
        &[0, 1, 2, 3],
        &[4, 1, 0],
        &[0, 1, 4],
        &[1, 0, 5],
        &[0, 1, 2],
        &[1, 6, 7],
        &[6, 8, 7],
        &[1, 11, 12, 10],
    ];
    for face in faces {
        let corners: Vec<VertId> = face.iter().map(|&index| verts[index]).collect();
        mesh.add_face(&corners).expect("fixture face");
    }
    mesh.add_edge(verts[7], verts[9]).expect("wire edge");
    mesh.update_normals();
    checked(&mesh);
    mesh
}

fn everything(mut mesh: EditMesh, mode: SelectMode) -> EditMesh {
    mesh.set_select_mode(mode);
    mesh.select_all(SelectAction::Select);
    mesh
}

fn fixtures() -> Vec<EditMesh> {
    let mut fixtures = Vec::new();
    for mode in [SelectMode::VERT, SelectMode::EDGE, SelectMode::FACE] {
        fixtures.push(everything(messy(), mode));
        fixtures.push(everything(EditMesh::new(), mode));
    }
    fixtures
}

fn exercise(name: &str, operator: impl Fn(&mut EditMesh)) {
    for (index, mut mesh) in fixtures().into_iter().enumerate() {
        operator(&mut mesh);
        if let Err(error) = mesh.validate() {
            panic!("{name} broke fixture {index}: {error}");
        }
    }
}

#[test]
fn deletes_survive_messy_input() {
    for kind in [
        DeleteType::Verts,
        DeleteType::Edges,
        DeleteType::Faces,
        DeleteType::EdgesFaces,
        DeleteType::OnlyFaces,
    ] {
        exercise("delete", |mesh| {
            delete(mesh, &DeleteOptions { kind }).expect("operator runs");
        });
    }
    exercise("delete loose", |mesh| {
        delete_loose(
            mesh,
            &DeleteLooseOptions {
                use_verts: true,
                use_edges: true,
                use_faces: true,
            },
        )
        .expect("operator runs");
    });
    exercise("delete edge loop", |mesh| {
        delete_edge_loop(mesh, &DeleteEdgeLoopOptions::default()).expect("operator runs");
    });
}

#[test]
fn dissolves_survive_messy_input() {
    exercise("dissolve verts", |mesh| {
        dissolve_verts(
            mesh,
            &DissolveVertsOptions {
                use_face_split: true,
                use_boundary_tear: true,
            },
        )
        .expect("operator runs");
    });
    exercise("dissolve edges", |mesh| {
        dissolve_edges(
            mesh,
            &DissolveEdgesOptions {
                use_face_split: true,
                ..DissolveEdgesOptions::default()
            },
        )
        .expect("operator runs");
    });
    exercise("dissolve faces", |mesh| {
        dissolve_faces(mesh, &DissolveFacesOptions { use_verts: true }).expect("operator runs");
    });
    exercise("dissolve mode", |mesh| {
        dissolve_mode(mesh, &DissolveModeOptions::default()).expect("operator runs");
    });
    exercise("limited dissolve", |mesh| {
        dissolve_limited(
            mesh,
            &LimitedDissolveOptions {
                angle_limit: 1.5,
                delimit: Delimit::all(),
                ..LimitedDissolveOptions::default()
            },
        )
        .expect("operator runs");
    });
    exercise("planar decimate", |mesh| {
        decimate_planar(mesh, 3.0, true, Delimit::empty());
    });
    exercise("degenerate dissolve", |mesh| {
        dissolve_degenerate(mesh, &DissolveDegenerateOptions { threshold: 0.6 })
            .expect("valid threshold");
    });
}

#[test]
fn merges_survive_messy_input() {
    for kind in [MergeType::Center, MergeType::Cursor, MergeType::Collapse] {
        exercise("merge", |mesh| {
            merge(
                mesh,
                &MergeOptions {
                    kind,
                    uvs: true,
                    ..MergeOptions::default()
                },
            )
            .expect("merge runs");
        });
    }
    exercise("collapse", |mesh| {
        collapse_edges(mesh).expect("collapse runs");
    });
    exercise("merge by distance", |mesh| {
        remove_doubles(
            mesh,
            &RemoveDoublesOptions {
                threshold: 0.75,
                ..RemoveDoublesOptions::default()
            },
        )
        .expect("valid threshold");
    });
    exercise("weld", |mesh| {
        let verts: Vec<VertId> = mesh.verts().collect();
        let targets: IdMap<VertId, VertId> = verts
            .chunks(2)
            .filter(|pair| pair.len() == 2)
            .map(|pair| (pair[1], pair[0]))
            .collect();
        weld(mesh, &targets, &WeldOptions::default()).expect("valid targets");
    });
    exercise("auto merge", |mesh| {
        let moved: IdSet<VertId> = mesh.verts().step_by(2).collect();
        auto_merge(
            mesh,
            &moved,
            &AutoMergeOptions {
                threshold: 0.75,
                use_centroid: true,
            },
        )
        .expect("valid threshold");
    });
    exercise("auto merge split", |mesh| {
        let moved: IdSet<VertId> = mesh.verts().step_by(3).collect();
        auto_merge_split(
            mesh,
            &moved,
            &AutoMergeOptions {
                threshold: 0.3,
                use_centroid: false,
            },
        )
        .expect("valid threshold");
    });
}

#[test]
fn reductions_survive_messy_input() {
    exercise("join triangles", |mesh| {
        join_triangles(
            mesh,
            &JoinTrianglesOptions {
                topology_influence: 2.0,
                ..JoinTrianglesOptions::default()
            },
        )
        .expect("operator runs");
    });
    exercise("unsubdivide", |mesh| {
        unsubdivide(mesh, &UnsubdivideOptions { iterations: 3 }).expect("valid iterations");
    });
    exercise("unsubdivide modifier", |mesh| {
        decimate_unsubdivide(mesh, 3);
    });
    exercise("decimate", |mesh| {
        decimate(
            mesh,
            &DecimateOptions {
                ratio: 0.1,
                use_symmetry: true,
                ..DecimateOptions::default()
            },
        )
        .expect("valid options");
    });
    exercise("decimate collapse", |mesh| {
        decimate_collapse(
            mesh,
            &Collapse {
                factor: 0.0,
                triangulate: true,
                ..Collapse::default()
            },
        );
    });
    exercise("duplicate faces", |mesh| {
        let faces: Vec<_> = mesh.faces().collect();
        remove_duplicate_faces(mesh, &faces);
    });
}

fn angle(rng: &mut Rng) -> f32 {
    rng.range(0.0, std::f32::consts::PI)
}

fn delimit(rng: &mut Rng) -> Delimit {
    Delimit::from_bits_truncate(rng.below(32) as u8)
}

fn distance(mesh: &EditMesh, rng: &mut Rng) -> f32 {
    let (low, high) = bounds(mesh);
    (high - low).length() * rng.range(0.0, 0.2)
}

fn verts_list(mesh: &EditMesh) -> Vec<VertId> {
    selected_verts(mesh).iter().collect()
}

fn join_settings(rng: &mut Rng) -> JoinTrianglesSettings {
    JoinTrianglesSettings {
        face_threshold: angle(rng),
        shape_threshold: angle(rng),
        topology_influence: rng.range(0.0, 2.0),
        uvs: rng.flip(),
        vcols: rng.flip(),
        seam: rng.flip(),
        sharp: rng.flip(),
        materials: rng.flip(),
        deselect_joined: rng.flip(),
    }
}

fn sweep_operators() -> Vec<(&'static str, EditOperator)> {
    vec![
        ("delete", |mesh, rng| {
            let options = DeleteOptions {
                kind: rng.pick(&DELETE_TYPES),
            };
            outcome(delete(mesh, &options))
        }),
        ("delete_loose", |mesh, rng| {
            let options = DeleteLooseOptions {
                use_verts: rng.flip(),
                use_edges: rng.flip(),
                use_faces: rng.flip(),
            };
            outcome(delete_loose(mesh, &options))
        }),
        ("delete_edge_loop", |mesh, rng| {
            let options = DeleteEdgeLoopOptions {
                use_face_split: rng.flip(),
            };
            outcome(delete_edge_loop(mesh, &options))
        }),
        ("dissolve_verts", |mesh, rng| {
            let options = DissolveVertsOptions {
                use_face_split: rng.flip(),
                use_boundary_tear: rng.flip(),
            };
            outcome(dissolve_verts(mesh, &options))
        }),
        ("dissolve_verts_geometry", |mesh, rng| {
            let verts = verts_list(mesh);
            let options = DissolveVertsOptions {
                use_face_split: rng.flip(),
                use_boundary_tear: rng.flip(),
            };
            dissolve_verts_geometry(mesh, &verts, &options);
            Ok(())
        }),
        ("dissolve_edges", |mesh, rng| {
            let options = DissolveEdgesOptions {
                use_verts: rng.flip(),
                angle_threshold: angle(rng),
                use_face_split: rng.flip(),
                use_preserve_quads: rng.flip(),
            };
            outcome(dissolve_edges(mesh, &options))
        }),
        ("dissolve_edges_geometry", |mesh, rng| {
            let edges: Vec<_> = selected_edges(mesh).iter().collect();
            let options = DissolveEdgesOptions {
                use_verts: rng.flip(),
                angle_threshold: angle(rng),
                use_face_split: rng.flip(),
                use_preserve_quads: rng.flip(),
            };
            dissolve_edges_geometry(mesh, &edges, &options);
            Ok(())
        }),
        ("dissolve_faces", |mesh, rng| {
            let options = DissolveFacesOptions {
                use_verts: rng.flip(),
            };
            outcome(dissolve_faces(mesh, &options))
        }),
        ("dissolve_faces_geometry", |mesh, rng| {
            let faces: Vec<_> = selected_faces(mesh).iter().collect();
            let options = DissolveFacesOptions {
                use_verts: rng.flip(),
            };
            dissolve_faces_geometry(mesh, &faces, &options);
            Ok(())
        }),
        ("dissolve_mode", |mesh, rng| {
            let options = DissolveModeOptions {
                use_verts: rng.flip().then(|| rng.flip()),
                angle_threshold: angle(rng),
                use_preserve_quads: rng.flip(),
                use_face_split: rng.flip(),
                use_boundary_tear: rng.flip(),
            };
            outcome(dissolve_mode(mesh, &options))
        }),
        ("dissolve_limited", |mesh, rng| {
            let options = LimitedDissolveOptions {
                angle_limit: angle(rng),
                use_dissolve_boundaries: rng.flip(),
                delimit: delimit(rng),
            };
            outcome(dissolve_limited(mesh, &options))
        }),
        ("dissolve_limited_geometry", |mesh, rng| {
            let verts = verts_list(mesh);
            let edges: Vec<_> = selected_edges(mesh).iter().collect();
            dissolve_limited_geometry(mesh, &verts, &edges, angle(rng), rng.flip(), delimit(rng));
            Ok(())
        }),
        ("decimate_planar", |mesh, rng| {
            decimate_planar(mesh, angle(rng), rng.flip(), delimit(rng));
            Ok(())
        }),
        ("dissolve_degenerate", |mesh, rng| {
            let options = DissolveDegenerateOptions {
                threshold: distance(mesh, rng),
            };
            outcome(dissolve_degenerate(mesh, &options))
        }),
        ("dissolve_degenerate_geometry", |mesh, rng| {
            let edges: Vec<_> = selected_edges(mesh).iter().collect();
            let threshold = distance(mesh, rng);
            dissolve_degenerate_geometry(mesh, &edges, threshold);
            Ok(())
        }),
        ("merge", |mesh, rng| {
            let (low, high) = bounds(mesh);
            let options = MergeOptions {
                kind: rng.pick(&MERGE_TYPES),
                uvs: rng.flip(),
                cursor: low.lerp(high, rng.unit()),
            };
            outcome(merge(mesh, &options))
        }),
        ("collapse_edges", |mesh, _| outcome(collapse_edges(mesh))),
        ("point_merge", |mesh, rng| {
            let verts = verts_list(mesh);
            let target = match rng.flip() {
                true => verts.first().copied(),
                false => None,
            };
            let position = rng.flip().then(|| rng.direction());
            point_merge(mesh, &verts, position, target);
            Ok(())
        }),
        ("merge_face_data_at", |mesh, rng| {
            let verts = verts_list(mesh);
            if let Some(target) = random_vert(mesh, rng) {
                merge_face_data_at(mesh, &verts, target);
            }
            Ok(())
        }),
        ("average_face_data", |mesh, _| {
            let verts = verts_list(mesh);
            average_face_data(mesh, &verts);
            Ok(())
        }),
        ("collapse_geometry", |mesh, rng| {
            let edges: Vec<_> = selected_edges(mesh).iter().collect();
            collapse_geometry(mesh, &edges, rng.flip());
            Ok(())
        }),
        ("remove_doubles", |mesh, rng| {
            let options = RemoveDoublesOptions {
                threshold: distance(mesh, rng),
                use_centroid: rng.flip(),
                use_unselected: rng.flip(),
                use_sharp_edge_from_normals: rng.flip(),
            };
            outcome(remove_doubles(mesh, &options))
        }),
        ("find_doubles_weld", |mesh, rng| {
            let verts = verts_list(mesh);
            let keep: IdSet<VertId> = verts.iter().copied().filter(|_| rng.chance(0.2)).collect();
            let options = FindDoublesOptions {
                distance: distance(mesh, rng),
                use_connected: rng.flip(),
            };
            let targets = find_doubles(mesh, &verts, &keep, &options);
            let options = WeldOptions {
                use_centroid: rng.flip(),
                average_vert_data: rng.flip(),
            };
            outcome(weld(mesh, &targets, &options))
        }),
        ("weld_random", |mesh, rng| {
            let verts: Vec<VertId> = mesh.verts().collect();
            let mut targets = IdMap::new();
            let mut used: IdSet<VertId> = IdSet::new();
            for &vert in &verts {
                if rng.chance(0.2) && !used.contains(vert) {
                    let target = verts[rng.below(verts.len())];
                    if target != vert && !targets.contains(target) {
                        targets.insert(vert, target);
                        used.insert(target);
                    }
                }
            }
            let options = WeldOptions {
                use_centroid: rng.flip(),
                average_vert_data: rng.flip(),
            };
            outcome(weld(mesh, &targets, &options))
        }),
        ("auto_merge", |mesh, rng| {
            let moved = selected_verts(mesh);
            let options = AutoMergeOptions {
                threshold: distance(mesh, rng),
                use_centroid: rng.flip(),
            };
            outcome(auto_merge(mesh, &moved, &options))
        }),
        ("auto_merge_split", |mesh, rng| {
            let moved = selected_verts(mesh);
            let options = AutoMergeOptions {
                threshold: distance(mesh, rng),
                use_centroid: rng.flip(),
            };
            outcome(auto_merge_split(mesh, &moved, &options))
        }),
        ("intersect_edges", |mesh, rng| {
            let moved = selected_verts(mesh);
            let threshold = distance(mesh, rng);
            drop(intersect_edges(mesh, &moved, threshold, rng.flip()));
            Ok(())
        }),
        ("join_triangles", |mesh, rng| {
            let settings = join_settings(rng);
            let options = JoinTrianglesOptions {
                face_threshold: rng.flip().then_some(settings.face_threshold),
                shape_threshold: rng.flip().then_some(settings.shape_threshold),
                topology_influence: settings.topology_influence,
                uvs: settings.uvs,
                vcols: settings.vcols,
                seam: settings.seam,
                sharp: settings.sharp,
                materials: settings.materials,
                deselect_joined: settings.deselect_joined,
            };
            outcome(join_triangles(mesh, &options))
        }),
        ("join_triangles_geometry", |mesh, rng| {
            let faces: Vec<_> = selected_faces(mesh).iter().collect();
            let settings = join_settings(rng);
            drop(join_triangles_geometry(mesh, &faces, &settings));
            Ok(())
        }),
        ("unsubdivide", |mesh, rng| {
            let options = UnsubdivideOptions {
                iterations: rng.int(1, 6),
            };
            outcome(unsubdivide(mesh, &options))
        }),
        ("unsubdivide_geometry", |mesh, rng| {
            let verts = verts_list(mesh);
            unsubdivide_geometry(mesh, &verts, rng.int(1, 6));
            Ok(())
        }),
        ("decimate_unsubdivide", |mesh, rng| {
            decimate_unsubdivide(mesh, rng.int(1, 4));
            Ok(())
        }),
        ("decimate", |mesh, rng| {
            let options = DecimateOptions {
                ratio: rng.unit(),
                use_vertex_group: rng.flip(),
                vertex_group_factor: rng.range(0.0, 1000.0),
                invert_vertex_group: rng.flip(),
                use_symmetry: rng.flip(),
                symmetry_axis: rng.pick(&[Axis::X, Axis::Y, Axis::Z]),
            };
            outcome(decimate(mesh, &options))
        }),
        ("decimate_collapse", |mesh, rng| {
            let weights = rng
                .flip()
                .then(|| (0..mesh.vert_capacity()).map(|_| rng.unit()).collect());
            let collapse = Collapse {
                factor: rng.unit(),
                weights,
                weight_factor: rng.range(0.0, 10.0),
                triangulate: rng.flip(),
                symmetry: rng.flip().then(|| rng.pick(&[Axis::X, Axis::Y, Axis::Z])),
                symmetry_epsilon: rng.range(0.0, 0.01),
            };
            decimate_collapse(mesh, &collapse);
            Ok(())
        }),
        ("remove_duplicate_faces", |mesh, _| {
            let faces: Vec<_> = selected_faces(mesh).iter().collect();
            remove_duplicate_faces(mesh, &faces);
            Ok(())
        }),
        ("collapse_short_edges", |mesh, rng| {
            let edges: Vec<_> = selected_edges(mesh).iter().collect();
            let threshold = distance(mesh, rng);
            collapse_short_edges(mesh, &edges, threshold);
            Ok(())
        }),
    ]
}

#[test]
fn reduce_operators_survive_corpus() {
    sweep_edit("reduce corpus", &corpus(), CASES, &sweep_operators());
}

#[test]
fn reduce_operators_survive_samples() {
    sweep_edit(
        "reduce samples",
        &sample_fixtures(),
        SAMPLE_CASES,
        &sweep_operators(),
    );
}

#[test]
fn reduce_operator_chains_survive() {
    let mut fixtures = corpus();
    fixtures.extend(sample_fixtures());
    sweep_chains(
        "reduce chains",
        &fixtures,
        CHAINS,
        CHAIN_LENGTH,
        &sweep_operators(),
    );
}
