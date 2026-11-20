use glam::{Vec2, Vec3, Vec4};
use rikka_mesh_modifiers::{
    CornerData, CornerMapping, DataTransfer, EdgeData, EdgeMapping, FaceData, FaceMapping, MixMode,
    ObjectRef, VertexData, VertexMapping,
};
use rikka_polymesh::{Domain, Polymesh};

use crate::oracle::scene::{
    DT_COLORS_FACE_NEAREST, DT_EDGES_NEAREST, DT_EDGES_POLY_NEAREST, DT_EDGES_VERT_NEAREST,
    DT_FACES_NEAREST, DT_FACES_PNORPROJ, DT_GROUPS_EDGE_INTERP_LOCAL, DT_GROUPS_INTERP_MIX,
    DT_GROUPS_NEAREST, DT_GROUPS_VNORPROJ, DT_NORMALS, DT_UV_LNORPROJ, DT_UV_NEAREST_POLYNOR,
    DT_UV_POLYINTERP,
};
use crate::oracle::transfer::{
    DT_EDGE_VNORPROJ_0_BEVEL, DT_EDGE_VNORPROJ_0_SHARP, DT_EDGE_VNORPROJ_1_BEVEL,
    DT_EDGE_VNORPROJ_1_SHARP, DT_EDGE_VNORPROJ_3_BEVEL, DT_EDGE_VNORPROJ_3_SHARP,
    DT_EDGE_VNORPROJ_TILTED_BEVEL, DT_EDGE_VNORPROJ_TILTED_SHARP, DT_NGON_DEST, DT_NGON_FACES_0,
    DT_NGON_FACES_2, DT_NGON_FACES_05, DT_NGON_UV_0, DT_NGON_UV_2, DT_SOURCE, DT_TILTED_DEST,
};
use crate::support::{
    GRID, Probe, Scene, assert_floats, assert_groups, assert_uvs, build, fixture, placement, run_in,
};

const SHORT_NORMAL_TOLERANCE: f32 = 5.0e-5;

const FINE_EDGES: &[[u32; 2]] = &[
    [0, 4],
    [0, 1],
    [1, 5],
    [4, 5],
    [1, 2],
    [2, 6],
    [5, 6],
    [2, 3],
    [3, 7],
    [6, 7],
    [4, 8],
    [5, 9],
    [8, 9],
    [6, 10],
    [9, 10],
    [7, 11],
    [10, 11],
    [8, 12],
    [9, 13],
    [12, 13],
    [10, 14],
    [13, 14],
    [11, 15],
    [14, 15],
];

fn source() -> Polymesh {
    let positions = (0..16)
        .map(|index| {
            let (x, y) = ((index % 4) as f32, (index / 4) as f32);
            Vec3::new(
                x / 3.0 * 1.2 - 0.1,
                y / 3.0 * 1.2 - 0.1,
                0.05 * x - 0.03 * y,
            )
        })
        .collect();
    let faces: Vec<Vec<u32>> = (0..9)
        .map(|index| {
            let (x, y) = (index % 3, index / 3);
            vec![
                y * 4 + x,
                y * 4 + x + 1,
                (y + 1) * 4 + x + 1,
                (y + 1) * 4 + x,
            ]
        })
        .collect();
    let face_refs: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
    let mut mesh = build(positions, FINE_EDGES, &face_refs);
    let a = mesh.vertex_groups.add("A");
    let b = mesh.vertex_groups.add("B");
    mesh.vertex_groups.ensure_weights(16);
    for point in 0..16 {
        mesh.vertex_groups.set_weight(point, a, point as f32 / 15.0);
        if point % 3 == 0 {
            mesh.vertex_groups
                .set_weight(point, b, 1.0 - point as f32 / 15.0);
        }
    }
    let uvs = mesh
        .corner_verts
        .iter()
        .map(|&point| {
            let position = mesh.positions[point as usize];
            Vec2::new(position.x * 0.7 + 0.05, position.y * 0.6 - 0.1)
        })
        .collect();
    mesh.add_uv_map("UVMap", uvs);
    mesh.uv_maps.set_active("UVMap");
    for (edge, sharp) in mesh.sharp_edges_mut().iter_mut().enumerate() {
        *sharp = edge % 3 == 0;
    }
    for (edge, weight) in mesh.edge_bevel_weights_mut().iter_mut().enumerate() {
        *weight = (edge % 5) as f32 / 4.0;
    }
    for (face, sharp) in mesh.sharp_faces_mut().iter_mut().enumerate() {
        *sharp = face % 2 == 0;
    }
    let colors = (0..16)
        .map(|point| {
            let t = point as f32 / 15.0;
            Vec4::new(t, 1.0 - t, 0.5, 1.0)
        })
        .collect();
    mesh.add_color_attribute("Col", Domain::Point, colors);
    mesh
}

fn destination() -> Polymesh {
    let mut mesh = fixture(&GRID);
    mesh.vertex_groups.add("A");
    mesh.vertex_groups.add("B");
    mesh.add_uv_map("UVMap", vec![Vec2::ZERO; mesh.corner_count()]);
    mesh.uv_maps.set_active("UVMap");
    mesh.sharp_edges_mut();
    mesh.edge_bevel_weights_mut();
    mesh.sharp_faces_mut();
    mesh.add_color_attribute("Col", Domain::Point, vec![Vec4::new(0.2, 0.2, 0.2, 1.0); 9]);
    mesh
}

fn scene() -> Scene {
    Scene {
        matrix: placement([0.0, 0.0, 0.1], [0.0; 3], [1.0; 3]),
        ..Scene::default()
    }
    .with_object(
        placement([0.05, -0.02, 0.01], [0.0, 0.0, 0.05], [1.0; 3]),
        Some(source()),
    )
}

fn transfer() -> DataTransfer {
    DataTransfer {
        object: Some(ObjectRef::new(0)),
        ..DataTransfer::default()
    }
}

fn vertex(data: VertexData, mapping: VertexMapping) -> DataTransfer {
    DataTransfer {
        use_vert_data: true,
        data_types_verts: data,
        vert_mapping: mapping,
        ..transfer()
    }
}

fn edge(mapping: EdgeMapping) -> DataTransfer {
    DataTransfer {
        use_edge_data: true,
        data_types_edges: EdgeData::SHARP | EdgeData::BEVEL_WEIGHT,
        edge_mapping: mapping,
        ..transfer()
    }
}

fn corner(data: CornerData, mapping: CornerMapping) -> DataTransfer {
    DataTransfer {
        use_loop_data: true,
        data_types_loops: data,
        loop_mapping: mapping,
        ..transfer()
    }
}

fn face(mapping: FaceMapping) -> DataTransfer {
    DataTransfer {
        use_poly_data: true,
        data_types_polys: FaceData::SMOOTH,
        poly_mapping: mapping,
        ..transfer()
    }
}

fn evaluate(transfer: DataTransfer) -> Polymesh {
    run_in(&destination(), transfer, &scene()).mesh
}

#[test]
fn vertex_groups_transfer_like_blender() {
    let cases: [(DataTransfer, &Probe); 4] = [
        (
            vertex(VertexData::VERTEX_GROUPS, VertexMapping::Nearest),
            &DT_GROUPS_NEAREST,
        ),
        (
            DataTransfer {
                mix_mode: MixMode::Mix,
                mix_factor: 0.5,
                ..vertex(
                    VertexData::VERTEX_GROUPS,
                    VertexMapping::FaceInterpolatedNearest,
                )
            },
            &DT_GROUPS_INTERP_MIX,
        ),
        (
            DataTransfer {
                use_object_transform: false,
                ..vertex(
                    VertexData::VERTEX_GROUPS,
                    VertexMapping::EdgeInterpolatedNearest,
                )
            },
            &DT_GROUPS_EDGE_INTERP_LOCAL,
        ),
        (
            vertex(
                VertexData::VERTEX_GROUPS,
                VertexMapping::FaceInterpolatedNormalProjected,
            ),
            &DT_GROUPS_VNORPROJ,
        ),
    ];
    for (transfer, probe) in cases {
        assert_groups(&evaluate(transfer), probe.groups);
    }
}

#[test]
fn colors_mix_like_blender() {
    let transfer = DataTransfer {
        mix_mode: MixMode::Add,
        mix_factor: 0.75,
        ..vertex(VertexData::COLOR, VertexMapping::FaceNearest)
    };
    let mesh = evaluate(transfer);
    let colors = mesh
        .attributes
        .values::<Vec4>("Col", Domain::Point)
        .expect("the color attribute survives");
    for (point, (actual, expected)) in colors.iter().zip(DT_COLORS_FACE_NEAREST.colors).enumerate()
    {
        assert!(
            actual.abs_diff_eq(Vec4::from(*expected), 1.0e-5),
            "point {point}: {actual} != {expected:?}"
        );
    }
}

#[test]
fn edge_data_transfers_like_blender() {
    let cases: [(DataTransfer, &Probe); 3] = [
        (edge(EdgeMapping::Nearest), &DT_EDGES_NEAREST),
        (edge(EdgeMapping::VertexNearest), &DT_EDGES_VERT_NEAREST),
        (edge(EdgeMapping::FaceNearest), &DT_EDGES_POLY_NEAREST),
    ];
    for (transfer, probe) in cases {
        let mesh = evaluate(transfer);
        assert_eq!(
            mesh.sharp_edges().expect("sharp edges exist"),
            probe.sharp_edges
        );
        assert_floats(
            mesh.edge_bevel_weights().expect("bevel weights exist"),
            probe.bevel_weights,
        );
    }
}

#[test]
fn uvs_transfer_like_blender() {
    for (mapping, probe) in [
        (CornerMapping::FaceInterpolatedNearest, &DT_UV_POLYINTERP),
        (CornerMapping::NearestFaceNormal, &DT_UV_NEAREST_POLYNOR),
        (
            CornerMapping::FaceInterpolatedNormalProjected,
            &DT_UV_LNORPROJ,
        ),
    ] {
        let mesh = evaluate(corner(CornerData::UV, mapping));
        assert_uvs(&mesh, "UVMap", probe.uvs);
    }
}

#[test]
fn custom_normals_transfer_like_blender() {
    let mesh = evaluate(corner(
        CornerData::CUSTOM_NORMAL,
        CornerMapping::NearestFaceNormal,
    ));
    for (corner, (actual, expected)) in mesh
        .corner_normals()
        .iter()
        .zip(DT_NORMALS.normals)
        .enumerate()
    {
        assert!(
            actual.abs_diff_eq(Vec3::from(*expected), SHORT_NORMAL_TOLERANCE),
            "corner {corner}: {actual} != {expected:?}"
        );
    }
}

#[test]
fn face_smoothing_transfers_like_blender() {
    for (mapping, probe) in [
        (FaceMapping::Nearest, &DT_FACES_NEAREST),
        (
            FaceMapping::FaceInterpolatedNormalProjected,
            &DT_FACES_PNORPROJ,
        ),
    ] {
        let mesh = evaluate(face(mapping));
        assert_eq!(
            mesh.sharp_faces().expect("sharp faces exist"),
            probe.sharp_faces
        );
    }
}

fn projected_scene(source: Polymesh) -> Scene {
    Scene {
        matrix: placement([0.0, 0.0, 0.1], [0.0; 3], [1.0; 3]),
        ..Scene::default()
    }
    .with_object(
        placement([0.05, -0.02, 0.01], [0.0, 0.0, 0.05], [1.0; 3]),
        Some(source),
    )
}

fn projected_source() -> Polymesh {
    DT_SOURCE.build(&[])
}

#[test]
fn edge_normal_projection_matches_blender() {
    let mut tilted = DT_TILTED_DEST.build(&[]);
    tilted.sharp_edges_mut();
    tilted.edge_bevel_weights_mut();
    let mut flat = fixture(&GRID);
    flat.sharp_edges_mut();
    flat.edge_bevel_weights_mut();
    let cases: [(&Polymesh, f32, &[bool], &[f32]); 4] = [
        (
            &flat,
            0.0,
            DT_EDGE_VNORPROJ_0_SHARP,
            DT_EDGE_VNORPROJ_0_BEVEL,
        ),
        (
            &flat,
            0.1,
            DT_EDGE_VNORPROJ_1_SHARP,
            DT_EDGE_VNORPROJ_1_BEVEL,
        ),
        (
            &flat,
            0.3,
            DT_EDGE_VNORPROJ_3_SHARP,
            DT_EDGE_VNORPROJ_3_BEVEL,
        ),
        (
            &tilted,
            0.2,
            DT_EDGE_VNORPROJ_TILTED_SHARP,
            DT_EDGE_VNORPROJ_TILTED_BEVEL,
        ),
    ];
    let scene = projected_scene(projected_source());
    for (destination, radius, sharp, bevel) in cases {
        let transfer = DataTransfer {
            ray_radius: radius,
            ..edge(EdgeMapping::EdgeInterpolatedNormalProjected)
        };
        let mesh = run_in(destination, transfer, &scene).mesh;
        assert_eq!(
            mesh.sharp_edges().expect("sharp edges exist"),
            sharp,
            "radius {radius}"
        );
        assert_floats(
            mesh.edge_bevel_weights().expect("bevel weights exist"),
            bevel,
        );
    }
}

#[test]
fn ngon_normal_projection_matches_blender() {
    let mut source = projected_source();
    for (face, sharp) in source.sharp_faces_mut().iter_mut().enumerate() {
        *sharp = face == 4;
    }
    let mut destination = DT_NGON_DEST.build(&[]);
    destination.sharp_faces_mut().fill(false);
    let scene = projected_scene(source);
    for (radius, expected) in [
        (0.0, DT_NGON_FACES_0),
        (0.2, DT_NGON_FACES_2),
        (0.05, DT_NGON_FACES_05),
    ] {
        let transfer = DataTransfer {
            ray_radius: radius,
            ..face(FaceMapping::FaceInterpolatedNormalProjected)
        };
        let mesh = run_in(&destination, transfer, &scene).mesh;
        assert_eq!(
            mesh.sharp_faces().expect("sharp faces exist"),
            expected,
            "radius {radius}"
        );
    }
}

#[test]
fn ngon_corner_projection_matches_blender() {
    let mut source = projected_source();
    let uvs = source
        .corner_verts
        .iter()
        .map(|&point| {
            let position = source.positions[point as usize];
            Vec2::new(position.x * 0.7 + 0.05, position.y * 0.6 - 0.1)
        })
        .collect();
    source.add_uv_map("UVMap", uvs);
    source.uv_maps.set_active("UVMap");
    let mut destination = DT_NGON_DEST.build(&[]);
    destination.add_uv_map("UVMap", vec![Vec2::ZERO; destination.corner_count()]);
    destination.uv_maps.set_active("UVMap");
    let scene = projected_scene(source);
    for (radius, expected) in [(0.0, DT_NGON_UV_0), (0.2, DT_NGON_UV_2)] {
        let transfer = DataTransfer {
            ray_radius: radius,
            ..corner(
                CornerData::UV,
                CornerMapping::FaceInterpolatedNormalProjected,
            )
        };
        let mesh = run_in(&destination, transfer, &scene).mesh;
        assert_uvs(&mesh, "UVMap", expected);
    }
}

#[test]
fn self_overlapping_face_projection_finishes_like_blender() {
    let mut destination = build(
        vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(2.0, 0.5, 0.3),
        ],
        &[
            [3, 0],
            [0, 1],
            [1, 2],
            [2, 3],
            [0, 2],
            [1, 3],
            [1, 4],
            [4, 2],
        ],
        &[&[0, 1, 2, 3], &[0, 2, 1, 3], &[1, 4, 2]],
    );
    destination.sharp_faces_mut().fill(true);
    let mut source = Polymesh::from_polygons(
        (0..16)
            .map(|index| Vec3::new((index % 4) as f32 - 1.5, (index / 4) as f32 - 1.5, 0.0))
            .collect(),
        (0..3u32).flat_map(|row| {
            (0..3u32).map(move |column| {
                let a = row * 4 + column;
                [a, a + 1, a + 5, a + 4]
            })
        }),
    );
    source.sharp_faces_mut().fill(false);
    let scene = Scene::default().with_object(placement([0.0; 3], [0.0; 3], [1.0; 3]), Some(source));
    let transfer = DataTransfer {
        ray_radius: 0.005606258,
        ..face(FaceMapping::FaceInterpolatedNormalProjected)
    };
    let mesh = run_in(&destination, transfer, &scene).mesh;
    assert_eq!(
        mesh.sharp_faces().expect("sharp faces exist"),
        &[false, true, false]
    );
}

#[test]
fn face_smoothing_creates_missing_destination_layer() {
    let destination = fixture(&GRID);
    assert!(destination.sharp_faces().is_none());
    let mut source = source();
    source.sharp_faces_mut().fill(true);
    let mesh = run_in(
        &destination,
        face(FaceMapping::Nearest),
        &projected_scene(source),
    )
    .mesh;
    let sharp = mesh.sharp_faces().expect("sharp faces were created");
    assert!(sharp.iter().all(|&value| value));
}
