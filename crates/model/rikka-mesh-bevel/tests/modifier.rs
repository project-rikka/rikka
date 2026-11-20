use rikka_mesh_bevel::{Affect, BevelInput, BevelOptions, Limit, WeightLayers, WidthType};

use crate::support::{build_case, expect_input};

const DEFAULT_ANGLE: f32 = 30.0 * (std::f64::consts::PI / 180.0) as f32;

#[test]
fn limit_methods_match_blender_modifier() {
    let group = Limit::VertexGroup {
        group: 0,
        invert: false,
    };
    let cases: [(&str, Affect, Limit, BevelOptions); 6] = [
        (
            "MOD_ANGLE",
            Affect::Edges,
            Limit::Angle(DEFAULT_ANGLE),
            BevelOptions {
                width: 0.15,
                segments: 2,
                ..BevelOptions::modifier()
            },
        ),
        (
            "MOD_WEIGHT",
            Affect::Edges,
            Limit::Weight,
            BevelOptions {
                width: 0.3,
                segments: 2,
                ..BevelOptions::modifier()
            },
        ),
        (
            "MOD_WEIGHT_VERTICES",
            Affect::Vertices,
            Limit::Weight,
            BevelOptions {
                affect: Affect::Vertices,
                width: 0.4,
                segments: 2,
                ..BevelOptions::modifier()
            },
        ),
        (
            "MOD_GROUP",
            Affect::Edges,
            group,
            BevelOptions {
                width: 0.2,
                segments: 2,
                ..BevelOptions::modifier()
            },
        ),
        (
            "MOD_GROUP_VERTICES",
            Affect::Vertices,
            group,
            BevelOptions {
                affect: Affect::Vertices,
                width: 0.3,
                segments: 1,
                ..BevelOptions::modifier()
            },
        ),
        (
            "MOD_NONE_PERCENT",
            Affect::Edges,
            Limit::None,
            BevelOptions {
                width_type: WidthType::Percent,
                width: 20.0,
                segments: 2,
                ..BevelOptions::modifier()
            },
        ),
    ];
    for (case, affect, limit, options) in &cases {
        expect_input(
            case,
            |mesh| {
                BevelInput::from_limit(mesh, *affect, *limit, &WeightLayers::default())
                    .expect("limit input")
            },
            options,
        );
    }
}

#[test]
fn unknown_vertex_group_is_rejected() {
    let mesh = build_case("MOD_ANGLE");
    let limit = Limit::VertexGroup {
        group: 3,
        invert: false,
    };
    assert!(BevelInput::from_limit(&mesh, Affect::Edges, limit, &WeightLayers::default()).is_err());
}
