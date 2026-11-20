use rikka_mesh_bevel::{BevelOptions, WidthType};

use crate::support::expect_operator;

#[test]
fn width_types_match_blender() {
    let cases: [(&str, WidthType, f32); 4] = [
        ("CUBE_LOOP_PERCENT", WidthType::Percent, 25.0),
        ("CUBE_LOOP_ABSOLUTE", WidthType::Absolute, 0.3),
        ("WEDGE_WIDTH", WidthType::Width, 0.2),
        ("WEDGE_DEPTH", WidthType::Depth, 0.2),
    ];
    for (case, width_type, width) in cases {
        expect_operator(
            case,
            &BevelOptions {
                width_type,
                width,
                segments: 2,
                ..BevelOptions::default()
            },
        );
    }
}
