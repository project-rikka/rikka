use rikka_mesh_bevel::BevelOptions;

use crate::support::{expect_operator, expect_summary};

fn options(width: f32, segments: u32) -> BevelOptions {
    BevelOptions {
        width,
        segments,
        ..BevelOptions::default()
    }
}

#[test]
fn edge_bevels_match_blender() {
    let cases: [(&str, BevelOptions); 16] = [
        ("CUBE_EDGE", options(0.2, 1)),
        ("CUBE_EDGE_THREE", options(0.3, 3)),
        ("CUBE_LOOP_TWO", options(0.25, 2)),
        ("CUBE_ALL_1", options(0.2, 1)),
        ("CUBE_ALL_2", options(0.2, 2)),
        ("CUBE_ALL_3", options(0.2, 3)),
        ("CUBE_ALL_4", options(0.2, 4)),
        ("PIPE", options(0.2, 4)),
        ("PIPE_THREE", options(0.2, 3)),
        ("GRID_BOUNDARY", options(0.2, 3)),
        ("GRID_TERMINAL", options(0.2, 2)),
        ("SPLIT_EDGE", options(0.2, 2)),
        ("SPLIT_WELD", options(0.2, 3)),
        ("LOOP_SLIDE_ON", options(0.2, 2)),
        ("LOOP_SLIDE_CHAIN", options(0.2, 2)),
        (
            "LOOP_SLIDE_OFF",
            BevelOptions {
                loop_slide: false,
                ..options(0.2, 2)
            },
        ),
    ];
    for (case, options) in &cases {
        expect_operator(case, options);
    }
}

#[test]
fn poles_and_cones_match_blender() {
    let cases: [(&str, BevelOptions); 2] = [
        ("SPHERE_ALL_3", options(0.08, 3)),
        (
            "CONE_ALL_3",
            BevelOptions {
                profile_shape: 0.7,
                ..options(0.1, 3)
            },
        ),
    ];
    for (case, options) in &cases {
        expect_operator(case, options);
    }
}

#[test]
fn complex_mesh_matches_blender_summary() {
    expect_summary("MONKEY_TOP", &options(0.02, 2));
    expect_summary(
        "MONKEY_ALL_CLAMP",
        &BevelOptions {
            clamp_overlap: true,
            ..options(0.05, 3)
        },
    );
}

#[test]
fn clamp_overlap_limits_width() {
    expect_operator(
        "CUBE_ALL_CLAMP",
        &BevelOptions {
            clamp_overlap: true,
            ..options(1.5, 2)
        },
    );
}

#[test]
fn zero_width_leaves_mesh_unchanged() {
    let mut mesh = crate::support::build_case("CUBE_EDGE");
    let before = mesh.to_polymesh();
    let output = rikka_mesh_bevel::bevel_selected(&mut mesh, &options(0.0, 3)).expect("bevel");
    assert!(output.is_empty());
    assert_eq!(mesh.to_polymesh().positions, before.positions);
    assert_eq!(mesh.face_count(), 6);
}
