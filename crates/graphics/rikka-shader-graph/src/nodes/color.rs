use crate::library::{Bypass, NodeSpec};
use crate::node::PortSpec;
use crate::nodes::{Function, NORMAL, PIXELS, POSITION, number, spatial, unit};
use crate::ops::Category;

pub(super) fn specs() -> Vec<NodeSpec> {
    let color = PortSpec::color("color", 0.8, 0.8, 0.8);
    vec![
        Function::new(
            "color.luminance_add",
            "Luminance-preserving add",
            "base + added with its HSV value replaced by the larger of the two inputs' values, so extra lights tint without blowing out (Star Rail).",
        )
        .category(Category::Color)
        .hosts(PIXELS)
        .value(
            &[
                PortSpec::color("base", 0.5, 0.5, 0.5),
                PortSpec::color("added", 0.0, 0.0, 0.0),
            ],
            spatial("value"),
            "rikka_graph_luminance_add({base}, {added})",
        ),
        Function::new(
            "color.saturated_shade",
            "Saturation-preserving shade",
            "ZZZ's colour normalization lerp(c, 0.4372 c + 0.5628 saturate(c / mean(c)), amount): keeps a shade's brightness while pushing its hue.",
        )
        .gated("amount", Bypass::Input("color"))
        .category(Category::Color)
        .hosts(PIXELS)
        .value(
            &[color, unit("amount", 1.0)],
            spatial("value"),
            "rikka_graph_saturated_shade({color}, {amount})",
        ),
        Function::new(
            "color.tone_curve",
            "Tone curve",
            "A filmic curve after adding lift × max(c - 0.6, 0): narkowicz (ACES fit 2.51/0.03/2.43/0.59/0.14), genshin (HoyoToon's 1.36/0.047/0.93/0.56/0.14), star_rail (2.8/0.4/2.1/0.5/1.5) or reinhard c/(1+c).",
        )
        .category(Category::Color)
        .hosts(PIXELS)
        .value(
            &[
                color,
                PortSpec::options("curve", &["narkowicz", "genshin", "star_rail", "reinhard"], 0),
                PortSpec::number("lift", 0.0).soft(0.0, 1.0),
            ],
            spatial("value"),
            "rikka_graph_tone_curve({color}, {curve}, {lift})",
        ),
        Function::new(
            "vector.bump",
            "Bump",
            "The normal tilted by the screen-space slope of height × distance metres (inverted when invert is on), blended in by strength.",
        )
        .category(Category::Vector)
        .value(
            &[
                PortSpec::number("height", 0.0),
                unit("strength", 1.0),
                PortSpec::number("distance", 0.1).soft(0.0, 1.0),
                PortSpec::toggle("invert", false),
                NORMAL,
            ],
            spatial("value"),
            "rikka_graph_bump($pixel.world_position, {normal}, {height}, {strength}, {distance}, {invert})",
        ),
        Function::new(
            "vector.from_tangent",
            "Tangent to world",
            "A tangent-space vector (x along the uv's u, y along v, z along normal) turned into world space, the frame built around normal from the mesh tangent and its handedness; not normalized.",
        )
        .category(Category::Vector)
        .value(
            &[PortSpec::vector("vector", 0.0, 0.0, 1.0), NORMAL],
            spatial("value"),
            "rikka_graph_from_tangent({vector}, {normal}, $pixel.tangent)",
        ),
        Function::new(
            "vector.to_tangent",
            "World to tangent",
            "A world vector expressed in the tangent frame around normal: x along the uv's u, y along v, z along normal.",
        )
        .category(Category::Vector)
        .value(
            &[PortSpec::vector("vector", 0.0, 0.0, 1.0), NORMAL],
            spatial("value"),
            "rikka_graph_to_tangent({vector}, {normal}, $pixel.tangent)",
        ),
        Function::new(
            "vector.curvature",
            "Curvature",
            "How fast normal turns across the screen per metre of position: mean is positive on ridges and negative in creases (1/r on a sphere of radius r), magnitude its unsigned size.",
        )
        .category(Category::Vector)
        .packed(
            &[NORMAL, POSITION],
            "rikka_npr_curvature($pixel, {normal}, {position})",
            "RikkaNprCurvature",
            &[(number("mean"), ".mean"), (number("magnitude"), ".magnitude")],
        ),
    ]
}
