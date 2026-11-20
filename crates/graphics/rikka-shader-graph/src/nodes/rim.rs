use crate::library::{Bypass, NodeSpec};
use crate::node::PortSpec;
use crate::nodes::{DEPTHS, Function, LIGHT, NORMAL, VIEW, number, plane, spatial, unit};
use crate::ops::Category;
use crate::types::{Constant, ValueType};

pub(super) fn specs() -> Vec<NodeSpec> {
    vec![
        Function::new(
            "npr.depth_rim",
            "Depth rim",
            "1 where the depth layer width pixels away (at 1080p, along the screen direction of the normal, only its horizontal side, or of toward) lies at least low…high metres behind this pixel, both divided by scale; scaling depth or sqrt_depth also shrinks the width with distance relative to scale and grows it with focal length; gap is that depth difference / scale.",
        )
        .packed(
            &[
                NORMAL,
                PortSpec::vector("toward", 0.0, 0.0, 0.0),
                PortSpec::number("width", 3.0).soft(0.0, 20.0),
                PortSpec::options("direction", &["horizontal", "normal", "toward"], 1),
                PortSpec::options("scaling", &["pixels", "depth", "sqrt_depth"], 0),
                PortSpec::options("source", DEPTHS, 0),
                PortSpec::number("low", 0.06).soft(0.0, 1.0),
                PortSpec::number("high", 0.12).soft(0.0, 1.0),
                PortSpec::number("scale", 1.0).soft(0.1, 4.0),
            ],
            "rikka_npr_depth_rim($pixel, $context, {normal}, {toward}, {width}, {direction}, {scaling}, {source}, {low}, {high}, {scale})",
            "RikkaNprDepthRim",
            &[(number("mask"), ".mask"), (number("gap"), ".gap")],
        ),
        Function::new(
            "npr.fresnel_rim",
            "Fresnel rim",
            "MToon's rim: saturate(1 - N·V + lift)^power.",
        )
        .value(
            &[
                NORMAL,
                VIEW,
                PortSpec::number("power", 5.0).soft(0.0, 32.0),
                PortSpec::number("lift", 0.0).soft(-1.0, 1.0),
            ],
            number("value"),
            "rikka_npr_fresnel_rim({normal}, {view}, {power}, {lift})",
        ),
        Function::new(
            "npr.split_rim",
            "Front and back rim",
            "Genshin's rim halves against r = normalize(V - L): front = (1 - N·r)^power on the side facing away from the light, back = (N·r)^power.",
        )
        .packed(
            &[
                NORMAL,
                VIEW,
                LIGHT,
                PortSpec::number("power", 7.5).soft(0.0, 32.0),
            ],
            "rikka_npr_split_rim({normal}, {view}, {light}, {power})",
            "RikkaNprSplitRim",
            &[(number("front"), ".front"), (number("back"), ".back")],
        ),
        Function::new(
            "npr.light_side",
            "Light-side mask",
            "Gates a rim to the lit side: smooth gives smoothstep(low, high, facing), uts saturate(rim - ((1 - HL(facing)) + level)); invert flips it.",
        )
        .value(
            &[
                PortSpec::number("facing", 0.0).range(-1.0, 1.0),
                PortSpec::number("low", 0.0).range(-1.0, 1.0),
                PortSpec::number("high", 0.5).range(-1.0, 1.0),
                PortSpec::toggle("invert", false),
                PortSpec::options("mode", &["smooth", "uts"], 0),
                unit("rim", 1.0),
                PortSpec::number("level", 0.0).range(-1.0, 1.0),
            ],
            number("value"),
            "rikka_npr_light_side({facing}, {low}, {high}, {invert}, {mode}, {rim}, {level})",
        ),
        Function::new(
            "npr.rim_shadow",
            "Rim shadow",
            "Star Rail's darkening rim multiplier: with r = N·normalize(V - offset) in view space, s = smoothstep(feather, 1, saturate((1 - r)^power × width)) × intensity / 4, the result lerp(1, 2 × color, s).",
        )
        .gated("intensity", Bypass::Constant(Constant::vec3(1.0, 1.0, 1.0)))
        .value(
            &[
                NORMAL,
                VIEW,
                PortSpec::vector("offset", 0.0, 0.0, 0.0),
                PortSpec::number("power", 1.0).soft(0.0, 16.0),
                PortSpec::number("width", 1.0).soft(0.0, 4.0),
                unit("feather", 0.5),
                PortSpec::number("intensity", 1.0).soft(0.0, 4.0),
                PortSpec::color("color", 0.5, 0.5, 0.5),
            ],
            spatial("value"),
            "rikka_npr_rim_shadow($context, {normal}, {view}, {offset}, {power}, {width}, {feather}, {intensity}, {color})",
        ),
        Function::new(
            "npr.rim_combine",
            "Rim combine",
            "rim × color × lerp(1, lighting, light_mix) added to, subtracted from or screened over base.",
        )
        .value(
            &[
                PortSpec::color("base", 0.0, 0.0, 0.0),
                unit("rim", 0.0),
                PortSpec::color("color", 1.0, 1.0, 1.0),
                PortSpec::color("lighting", 1.0, 1.0, 1.0),
                unit("light_mix", 1.0),
                PortSpec::options("mode", &["add", "subtract", "screen"], 0),
            ],
            spatial("value"),
            "rikka_npr_rim_combine({base}, {rim}, {color}, {lighting}, {light_mix}, {mode})",
        ),
        Function::new(
            "npr.screen",
            "Screen position",
            "This pixel on screen: uv 0…1 from the top left, pixel coordinates, depth in metres along the view, focal = projection scale ÷ 2.414 (1 at a 45° vertical field of view), height in pixels and aspect width/height.",
        )
        .packed(
            &[],
            "rikka_npr_screen($pixel, $context)",
            "RikkaNprScreen",
            &[
                (PortSpec::output("uv", ValueType::Float2), ".uv"),
                (PortSpec::output("pixel", ValueType::Float2), ".pixel"),
                (number("depth"), ".depth"),
                (number("focal"), ".focal"),
                (number("height"), ".height"),
                (number("aspect"), ".aspect"),
            ],
        ),
        Function::new(
            "npr.scene_depth",
            "Scene depth",
            "The linear depth in metres of the scene, hair or face depth layer offset pixels away (1e6 where empty or unavailable), and gap = that depth - this pixel's depth.",
        )
        .packed(
            &[plane("offset", 0.0, 0.0), PortSpec::options("source", DEPTHS, 0)],
            "rikka_npr_scene_depth($pixel, $context, {offset}, {source})",
            "RikkaNprSceneDepth",
            &[(number("depth"), ".depth"), (number("gap"), ".gap")],
        ),
        Function::new(
            "vector.view_transform",
            "View transform",
            "A world vector moved into view space (x right, y up, z towards the camera) or back; point also applies the camera position.",
        )
        .category(Category::Vector)
        .value(
            &[
                PortSpec::vector("vector", 0.0, 0.0, 0.0),
                PortSpec::options("mode", &["world_to_view", "view_to_world"], 0),
                PortSpec::toggle("point", false),
            ],
            spatial("value"),
            "rikka_npr_transform($context, {vector}, {mode}, {point})",
        ),
    ]
}
