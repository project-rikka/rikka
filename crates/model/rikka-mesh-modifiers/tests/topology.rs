use glam::{Vec2, Vec3};
use rikka_mesh_modifiers::{Array, EdgeSplit, FitType, Triangulate, Weld, WeldMode};
use rikka_polymesh::{QuadMethod, ordered};

use crate::oracle::basic::{
    ARRAY_FIT_CONSTANT, ARRAY_MERGE, EDGE_SPLIT, TRIANGULATE_BEAUTY, TRIANGULATE_FIXED, WELD_ALL,
    WELD_CONNECTED,
};
use crate::support::{
    CUBE, CYLINDER, GRID, assert_edges, assert_matches, assert_uvs, build, fixture, moved, run,
    with_uvs,
};

#[test]
fn array_merges_neighbouring_copies() {
    let array = Array {
        count: 3,
        use_merge_vertices: true,
        ..Array::default()
    };
    assert_matches(&run(&fixture(&CUBE), array).mesh, &ARRAY_MERGE);
}

#[test]
fn array_fits_length_and_offsets_uvs() {
    let array = Array {
        fit_type: FitType::FitLength,
        fit_length: 3.2,
        use_relative_offset: false,
        use_constant_offset: true,
        constant_offset_displace: Vec3::new(1.0, 0.25, 0.0),
        offset_uv: Vec2::new(0.1, 0.0),
        ..Array::default()
    };
    let evaluated = run(&with_uvs(fixture(&GRID)), array).mesh;
    assert_matches(&evaluated, &ARRAY_FIT_CONSTANT);
    assert_uvs(&evaluated, "UVMap", ARRAY_FIT_CONSTANT.uvs);
}

#[test]
fn weld_merges_close_points() {
    let points = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0004, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [2.0, 1.0, 0.0],
        [1.0004, 1.0, 0.0],
    ];
    let mesh = build(
        points.iter().copied().map(Vec3::from).collect(),
        &[
            [0, 3],
            [0, 1],
            [1, 2],
            [2, 3],
            [4, 7],
            [4, 5],
            [5, 6],
            [6, 7],
        ],
        &[&[0, 1, 2, 3], &[4, 5, 6, 7]],
    );
    let weld = Weld {
        merge_threshold: 0.001,
        ..Weld::default()
    };
    let evaluated = run(&mesh, weld).mesh;
    assert_matches(&evaluated, &WELD_ALL);
    assert_edges(&evaluated, &WELD_ALL);
}

#[test]
fn connected_weld_collapses_short_edges() {
    let cube = moved(fixture(&CUBE), Vec3::ZERO, Vec3::new(0.2, 1.0, 1.0));
    let weld = Weld {
        merge_threshold: 0.5,
        mode: WeldMode::Connected,
        ..Weld::default()
    };
    let evaluated = run(&cube, weld).mesh;
    assert_matches(&evaluated, &WELD_CONNECTED);
    assert_edges(&evaluated, &WELD_CONNECTED);
}

#[test]
fn triangulate_matches_blender() {
    for (method, mesh, oracle) in [
        (QuadMethod::Beauty, fixture(&CYLINDER), &TRIANGULATE_BEAUTY),
        (QuadMethod::Fixed, fixture(&CUBE), &TRIANGULATE_FIXED),
    ] {
        let triangulate = Triangulate {
            quad_method: method,
            ..Triangulate::default()
        };
        let evaluated = run(&mesh, triangulate).mesh;
        assert_matches(&evaluated, oracle);
        assert_edges(&evaluated, oracle);
    }
}

#[test]
fn edge_split_separates_sharp_edges() {
    let split = EdgeSplit {
        split_angle: 40.0f32.to_radians(),
        ..EdgeSplit::default()
    };
    let evaluated = run(&fixture(&CYLINDER), split).mesh;
    assert_matches(&evaluated, &EDGE_SPLIT);
    let edges: Vec<[u32; 2]> = evaluated.edges.iter().map(|&edge| ordered(edge)).collect();
    let expected: Vec<[u32; 2]> = EDGE_SPLIT.edges.iter().map(|&edge| ordered(edge)).collect();
    assert_eq!(edges, expected);
}
