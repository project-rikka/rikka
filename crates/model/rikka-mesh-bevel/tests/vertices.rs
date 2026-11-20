use rikka_mesh_bevel::{Affect, BevelOptions, WidthType};

use crate::support::expect_operator;

fn vertex(width: f32, segments: u32) -> BevelOptions {
    BevelOptions {
        affect: Affect::Vertices,
        width,
        segments,
        ..BevelOptions::default()
    }
}

#[test]
fn vertex_bevels_match_blender() {
    let cases: [(&str, BevelOptions); 13] = [
        ("CUBE_VERTEX_ONE", vertex(0.4, 1)),
        ("CUBE_VERTEX_THREE", vertex(0.4, 3)),
        (
            "CUBE_VERTEX_PERCENT",
            BevelOptions {
                width_type: WidthType::Percent,
                ..vertex(30.0, 2)
            },
        ),
        (
            "CUBE_VERTEX_WIDTH",
            BevelOptions {
                width_type: WidthType::Width,
                ..vertex(0.4, 2)
            },
        ),
        (
            "CUBE_VERTEX_DEPTH",
            BevelOptions {
                width_type: WidthType::Depth,
                ..vertex(0.4, 2)
            },
        ),
        ("CUBE_VERTEX_ALL", vertex(0.3, 2)),
        (
            "CUBE_VERTEX_CLAMP",
            BevelOptions {
                clamp_overlap: true,
                ..vertex(1.5, 2)
            },
        ),
        ("GRID_BOUNDARY_VERTEX", vertex(0.3, 2)),
        ("BOWTIE_VERTEX", vertex(0.3, 2)),
        ("FIN_VERTEX", vertex(0.3, 1)),
        ("SPLIT_VERTEX", vertex(0.3, 3)),
        ("WIRE_VERTEX", vertex(0.3, 3)),
        ("ICO_VERTEX_ALL", vertex(0.15, 2)),
    ];
    for (case, options) in &cases {
        expect_operator(case, options);
    }
}
