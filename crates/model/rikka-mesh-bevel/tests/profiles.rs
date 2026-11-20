use glam::Vec2;
use rikka_mesh_bevel::{
    BevelOptions, CurveProfile, HandleType, InnerMiter, Intersection, OuterMiter, ProfilePoint,
    ProfilePreset, ProfileType,
};

use crate::support::expect_operator;

fn shaped(width: f32, segments: u32, profile_shape: f32) -> BevelOptions {
    BevelOptions {
        width,
        segments,
        profile_shape,
        ..BevelOptions::default()
    }
}

fn custom(width: f32, segments: u32, profile: CurveProfile) -> BevelOptions {
    BevelOptions {
        width,
        segments,
        profile_type: ProfileType::Custom(profile),
        ..BevelOptions::default()
    }
}

fn point(x: f32, y: f32, handle_type: HandleType) -> ProfilePoint {
    ProfilePoint::new(Vec2::new(x, y), handle_type)
}

fn three_point_profile() -> CurveProfile {
    CurveProfile::new(vec![
        point(1.0, 0.0, HandleType::Vector),
        point(0.6, 0.6, HandleType::Auto),
        point(0.0, 1.0, HandleType::Vector),
    ])
    .expect("profile")
}

#[test]
fn superellipse_shapes_match_blender() {
    let cases: [(&str, BevelOptions); 8] = [
        ("CUBE_LOOP_FOUR_SQUARE", shaped(0.25, 4, 1.0)),
        ("CUBE_ALL_3_P25", shaped(0.2, 3, 0.25)),
        ("CUBE_ALL_4_P75", shaped(0.2, 4, 0.75)),
        ("CUBE_ALL_4_SQUARE", shaped(0.2, 4, 1.0)),
        ("CUBE_ALL_3_SQUARE_IN", shaped(0.2, 3, 0.0)),
        ("CUBE_ALL_4_SQUARE_IN", shaped(0.2, 4, 0.0)),
        ("CUBE_ALL_5_P15", shaped(0.15, 5, 0.15)),
        ("CUBE_EDGE_P15", shaped(0.3, 3, 0.15)),
    ];
    for (case, options) in &cases {
        expect_operator(case, options);
    }
}

#[test]
fn cutoff_intersection_matches_blender() {
    expect_operator(
        "CUBE_ALL_CUTOFF",
        &BevelOptions {
            intersection: Intersection::Cutoff,
            ..shaped(0.2, 3, 0.5)
        },
    );
}

#[test]
fn custom_profiles_match_blender() {
    let even = CurveProfile::new(vec![
        point(1.0, 0.0, HandleType::Auto),
        point(0.8, 0.6, HandleType::Auto),
        point(0.0, 1.0, HandleType::Auto),
    ])
    .expect("profile")
    .with_sample_even_lengths(true);
    let cases: [(&str, BevelOptions); 9] = [
        ("CUSTOM_LOOP", custom(0.3, 4, three_point_profile())),
        ("CUSTOM_ALL", custom(0.2, 3, three_point_profile())),
        ("CUSTOM_PIPE", custom(0.2, 4, three_point_profile())),
        (
            "CUSTOM_STEPS",
            custom(
                0.3,
                6,
                CurveProfile::from_preset(ProfilePreset::Steps, 4).with_sample_straight_edges(true),
            ),
        ),
        ("CUSTOM_EVEN", custom(0.3, 5, even)),
        (
            "CUSTOM_SUPPORTS",
            custom(
                0.3,
                6,
                CurveProfile::from_preset(ProfilePreset::SupportLoops, 6),
            ),
        ),
        (
            "CUSTOM_CORNICE",
            custom(0.3, 8, CurveProfile::from_preset(ProfilePreset::Cornice, 8)),
        ),
        (
            "CUSTOM_CROWN",
            custom(
                0.3,
                12,
                CurveProfile::from_preset(ProfilePreset::CrownMolding, 12)
                    .with_sample_straight_edges(true),
            ),
        ),
        (
            "CUSTOM_MITER",
            BevelOptions {
                miter_outer: OuterMiter::Arc,
                miter_inner: InnerMiter::Arc,
                ..custom(0.15, 3, three_point_profile())
            },
        ),
    ];
    for (case, options) in &cases {
        expect_operator(case, options);
    }
}

#[test]
fn profile_samples_span_the_endpoints() {
    let samples = CurveProfile::from_preset(ProfilePreset::Line, 0).sample(4);
    assert_eq!(samples.len(), 5);
    assert!(samples[0].distance(Vec2::new(1.0, 0.0)) < 1.0e-6);
    assert!(samples[2].distance(Vec2::new(0.5, 0.5)) < 1.0e-5);
    assert!(samples[4].distance(Vec2::new(0.0, 1.0)) < 1.0e-5);
}

#[test]
fn profile_needs_two_points() {
    assert!(CurveProfile::new(vec![point(1.0, 0.0, HandleType::Auto)]).is_err());
}
