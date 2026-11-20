use rikka_mesh_bevel::{BevelOptions, InnerMiter, OuterMiter};

use crate::support::expect_operator;

#[test]
fn miters_match_blender_at_reflex_and_t_corners() {
    use InnerMiter as I;
    use OuterMiter as O;
    let cases: [(&str, OuterMiter, InnerMiter); 12] = [
        ("L_SHARP_SHARP", O::Sharp, I::Sharp),
        ("L_SHARP_ARC", O::Sharp, I::Arc),
        ("L_PATCH_SHARP", O::Patch, I::Sharp),
        ("L_PATCH_ARC", O::Patch, I::Arc),
        ("L_ARC_SHARP", O::Arc, I::Sharp),
        ("L_ARC_ARC", O::Arc, I::Arc),
        ("T_SHARP_SHARP", O::Sharp, I::Sharp),
        ("T_SHARP_ARC", O::Sharp, I::Arc),
        ("T_PATCH_SHARP", O::Patch, I::Sharp),
        ("T_PATCH_ARC", O::Patch, I::Arc),
        ("T_ARC_SHARP", O::Arc, I::Sharp),
        ("T_ARC_ARC", O::Arc, I::Arc),
    ];
    for (case, miter_outer, miter_inner) in cases {
        expect_operator(
            case,
            &BevelOptions {
                width: 0.15,
                segments: 2,
                miter_outer,
                miter_inner,
                spread: 0.05,
                ..BevelOptions::default()
            },
        );
    }
}
