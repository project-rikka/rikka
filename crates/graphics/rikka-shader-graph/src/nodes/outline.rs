use crate::library::NodeSpec;
use crate::node::{DirectionSpace, Input, PortSpec};
use crate::nodes::{
    CHANNELS, Function, NORMAL, POSITION, VERTEX_COLOR, VIEW, number, spatial, unit,
};
use crate::types::ValueType;

pub(super) fn specs() -> Vec<NodeSpec> {
    vec![
        Function::new(
            "npr.outline_width",
            "Outline width",
            "Hull width in metres for the outline_width output: world width × scale, pixels width pixels on screen, distance width × scale × a scale piecewise-linear over the eye distance (depth / scale, normalized to a 45° field of view when fov is on) through near, middle and far, or screen width × depth / focal (a share of half the screen height, MToon's screen coordinates and MMD edges); times the vertex colour channel and multiplier, then clamped to min_pixels…max_pixels on screen where those are above 0.",
        )
        .value(
            &[
                POSITION,
                VERTEX_COLOR,
                PortSpec::number("width", 0.002).soft(0.0, 0.01),
                PortSpec::options("mode", &["world", "pixels", "distance", "screen"], 0),
                PortSpec::number("min_pixels", 0.0).soft(0.0, 4.0),
                PortSpec::number("max_pixels", 0.0).soft(0.0, 16.0),
                PortSpec::toggle("fov", true),
                PortSpec::number("scale", 1.0).soft(0.1, 4.0),
                PortSpec::number("near", 1.0).soft(0.0, 10.0),
                PortSpec::number("middle", 3.0).soft(0.0, 30.0),
                PortSpec::number("far", 10.0).soft(0.0, 100.0),
                PortSpec::number("near_scale", 1.0).soft(0.0, 4.0),
                PortSpec::number("middle_scale", 1.5).soft(0.0, 8.0),
                PortSpec::number("far_scale", 2.5).soft(0.0, 16.0),
                PortSpec::options("channel", CHANNELS, 0),
                PortSpec::number("multiplier", 1.0).soft(0.0, 2.0),
            ],
            number("value"),
            "rikka_npr_outline_width($context, {position}, {vertex_color}, {width}, {mode}, {min_pixels}, {max_pixels}, {fov}, {scale}, {near}, {middle}, {far}, {near_scale}, {middle_scale}, {far_scale}, {channel}, {multiplier})",
        ),
        Function::new(
            "npr.outline_direction",
            "Outline direction",
            "The direction the hull grows for the outline_direction output: the baked smoothed normal, the normal, the tangent (smoothed normals baked into tangents), a tangent-space normal in the vertex colour (rgb × 2 - 1) or custom; flatten sets its view-space z to flatten_z and renormalizes x and y, so the outline grows across the screen.",
        )
        .value(
            &[
                NORMAL,
                PortSpec::context("smoothed", ValueType::Float3, Input::SmoothedNormal),
                PortSpec::context(
                    "tangent",
                    ValueType::Float3,
                    Input::Tangent(DirectionSpace::World),
                ),
                VERTEX_COLOR,
                PortSpec::vector("custom", 0.0, 0.0, 0.0),
                PortSpec::options(
                    "source",
                    &["smoothed", "normal", "tangent", "vertex_color", "custom"],
                    0,
                ),
                PortSpec::toggle("flatten", false),
                PortSpec::number("flatten_z", 0.01).range(-1.0, 1.0),
            ],
            spatial("value"),
            "rikka_npr_outline_direction($context, {normal}, {smoothed}, {tangent}, {vertex_color}, {custom}, {source}, {flatten}, {flatten_z})",
        ),
        Function::new(
            "npr.outline_depth",
            "Outline depth push",
            "How far the hull moves away from the camera for the outline_depth output, in metres: metres offset × scale, or relative depth × lerp(0.1 × offset, offset, saturate(1 - eye distance)) × scale (Genshin, eye distance as for outline width); times mask.",
        )
        .value(
            &[
                POSITION,
                PortSpec::number("offset", 0.0).soft(0.0, 0.05),
                PortSpec::options("mode", &["metres", "relative"], 0),
                PortSpec::number("scale", 1.0).soft(0.1, 4.0),
                PortSpec::toggle("fov", true),
                unit("mask", 1.0),
            ],
            number("value"),
            "rikka_npr_outline_depth($context, {position}, {offset}, {mode}, {scale}, {fov}, {mask})",
        ),
        Function::new(
            "npr.outline_fade",
            "Outline profile fade",
            "Star Rail's face outline fade: smoothstep(-0.02, 0, 1 - max(0, F·V)^power - mask), so masked lines (mouth) show only in profile.",
        )
        .value(
            &[
                VIEW,
                unit("mask", 0.0),
                PortSpec::number("power", 0.8).soft(0.1, 4.0),
            ],
            number("value"),
            "rikka_npr_outline_fade($context, {view}, {mask}, {power})",
        ),
        Function::new(
            "npr.flatten",
            "Perspective flatten",
            "A displacement that scales each point's view-space x and y by lerp(1, z / pivot z, amount), so the figure projects as if all of it stood at the pivot depth (the head, the draw's centre or center); it moves the geometry itself, lighting included.",
        )
        .value(
            &[
                POSITION,
                unit("amount", 0.0),
                PortSpec::options("pivot", &["head", "object", "custom"], 0),
                PortSpec::vector("center", 0.0, 0.0, 0.0),
            ],
            spatial("value"),
            "rikka_npr_flatten($context, {position}, {amount}, {pivot}, {center})",
        ),
    ]
}
