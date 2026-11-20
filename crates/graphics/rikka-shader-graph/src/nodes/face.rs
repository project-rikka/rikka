use rikka_slang::TextureKind;

use crate::library::{Bypass, NodeSpec};
use crate::node::PortSpec;
use crate::nodes::{Function, LIGHT, NORMAL, POSITION, UV, VIEW, number, spatial, texture, unit};
use crate::types::{Constant, ValueType};

pub(super) fn specs() -> Vec<NodeSpec> {
    vec![
        Function::new(
            "npr.head",
            "Head frame",
            "The figure's head in world space: centre, radius in metres, forward, up and right (up × forward); present is 0 for draws without a head (then the draw's bounding sphere and world axes); scale = radius / 0.1 m, the figure's size relative to a standard head.",
        )
        .packed(
            &[],
            "rikka_npr_head($context)",
            "RikkaNprHead",
            &[
                (spatial("center"), ".center"),
                (number("radius"), ".radius"),
                (spatial("forward"), ".forward"),
                (spatial("up"), ".up"),
                (spatial("right"), ".right"),
                (number("present"), ".present"),
                (number("scale"), ".scale"),
            ],
        ),
        Function::new(
            "npr.face_sample",
            "Face SDF sample",
            "Reads a face shadow map channel at uv and at the mirrored uv (1-u, v), once per pixel, for SDF face shadow.",
        )
        .packed(
            &[
                texture("texture", TextureKind::Data),
                UV,
                PortSpec::options("channel", &["r", "g", "b", "a"], 3),
            ],
            "rikka_npr_face_sample($pixel, {texture}, {uv}, {channel})",
            "RikkaNprFaceSample",
            &[(number("sdf"), ".sdf"), (number("mirrored"), ".mirrored")],
        ),
        Function::new(
            "npr.face_sdf",
            "SDF face shadow",
            "Face shadow from an SDF map: the head-plane light picks sdf or mirrored (mirror left uses mirrored when the light is on the -right side, right the opposite), threshold = 1 - HL(F·L) remapped over rotation…rotation+1 when rotation ≠ 0, lit = smoothstep(threshold ± softness, sdf) (a step at softness 0), zeroed where F·L < cutoff unless cutoff is -1.",
        )
        .packed(
            &[
                unit("sdf", 0.5),
                unit("mirrored", 0.5),
                LIGHT,
                PortSpec::options("mirror", &["left", "right"], 0),
                PortSpec::number("softness", 0.02).soft(0.0, 0.2),
                PortSpec::number("rotation", 0.0).range(-1.0, 1.0),
                PortSpec::number("cutoff", -1.0).range(-1.0, 1.0),
            ],
            "rikka_npr_face_sdf($context, {sdf}, {mirrored}, {light}, {mirror}, {softness}, {rotation}, {cutoff})",
            "RikkaNprFace",
            &[
                (number("lit"), ".lit"),
                (number("threshold"), ".threshold"),
                (number("sdf"), ".sdf"),
                (number("upright"), ".upright"),
            ],
        ),
        Function::new(
            "npr.face_procedural",
            "Procedural face shadow",
            "A face shadow without a map, in head-radius units: the boundary sweeps in vertically from the side away from the light (width stretches the face sideways, wrap pushes it towards the dark side), a nose wedge from nose_top to nose_tip and a lift below chin_height; lit = smoothstep(±softness, value).",
        )
        .packed(
            &[
                POSITION,
                LIGHT,
                PortSpec::number("width", 0.75).soft(0.2, 1.5),
                PortSpec::number("wrap", 0.15).soft(0.0, 0.6),
                PortSpec::number("softness", 0.06).soft(0.0, 0.3),
                unit("nose_shadow", 1.0),
                PortSpec::number("nose_width", 0.2).soft(0.0, 1.0),
                PortSpec::number("nose_top", -0.3).soft(-1.0, 0.5),
                PortSpec::number("nose_tip", -0.55).soft(-1.5, 0.0),
                PortSpec::number("chin", 0.1).soft(0.0, 0.5),
                PortSpec::number("chin_height", -1.0).soft(-2.0, 0.0),
            ],
            "rikka_npr_face_procedural($context, {position}, {light}, {width}, {wrap}, {softness}, {nose_shadow}, {nose_width}, {nose_top}, {nose_tip}, {chin}, {chin_height})",
            "RikkaNprProceduralFace",
            &[
                (number("lit"), ".lit"),
                (number("value"), ".value"),
                (number("upright"), ".upright"),
            ],
        ),
        Function::new(
            "npr.nose_line",
            "Nose line",
            "A nose line shown near the frontal view: head_facing step(threshold - mask, |F·V|^power) (Star Rail, threshold 1.03), normal_facing step(threshold, mask × (N·V)^power) (threshold about 0.1).",
        )
        .value(
            &[
                unit("mask", 0.0),
                PortSpec::options("mode", &["head_facing", "normal_facing"], 0),
                PortSpec::number("power", 4.0).soft(0.1, 32.0),
                PortSpec::number("threshold", 1.03).soft(0.0, 1.1),
                NORMAL,
                VIEW,
            ],
            number("value"),
            "rikka_npr_nose_line($context, {mask}, {mode}, {power}, {threshold}, {normal}, {view})",
        ),
        Function::new(
            "npr.face_highlight",
            "Face highlight",
            "ZZZ cheek and nose highlight: smoothstep(threshold ± window, saturate(mask - 0.5)) × saturate(N·H)^power × strength.",
        )
        .gated("strength", Bypass::Constant(Constant::scalar(0.0)))
        .value(
            &[
                unit("mask", 0.0),
                PortSpec::number("threshold", 0.0).soft(-1.0, 1.0),
                NORMAL,
                LIGHT,
                VIEW,
                PortSpec::number("window", 0.75).soft(0.0, 1.0),
                PortSpec::number("power", 10.0).soft(1.0, 64.0),
                PortSpec::number("strength", 10.0).soft(0.0, 20.0),
            ],
            number("value"),
            "rikka_npr_face_highlight({mask}, {threshold}, {normal}, {light}, {view}, {window}, {power}, {strength})",
        ),
        Function::new(
            "npr.eye_through_hair",
            "Eye-through-hair alpha",
            "Hair alpha over the eyes rising as the view leaves the face's front: max((1 - V·F in the head's horizontal plane)/across, (1 - V·F in its vertical plane)/down, lowest); across 0.658 = 1 - cos 70°, down 0.293 = 1 - cos 45°.",
        )
        .value(
            &[
                VIEW,
                PortSpec::number("across", 0.658).range(0.01, 2.0),
                PortSpec::number("down", 0.293).range(0.01, 2.0),
                unit("lowest", 0.0),
            ],
            number("value"),
            "rikka_npr_eye_through_hair($context, {view}, {across}, {down}, {lowest})",
        ),
        Function::new(
            "npr.see_through",
            "See-through mask",
            "1 where this face part lies within reach metres behind the frontmost hair and is the frontmost face part (the eye-through-hair test of the see-through pass), fading over the last 40 % of reach; 0 without hair and face depth layers.",
        )
        .value(
            &[PortSpec::number("reach", 0.05).soft(0.0, 0.5)],
            number("value"),
            "rikka_npr_see_through($pixel, $context, {reach})",
        ),
        Function::new(
            "npr.skin_reject",
            "See-through skin rejection",
            "An alpha factor for the see-through pass: 1 - amount × how much color looks like pale anime skin (a narrow YCbCr test), so eyelid skin painted into an eye texture stays behind the bangs while the iris shows; 1 in every other pass.",
        )
        .gated("amount", Bypass::Constant(Constant::scalar(1.0)))
        .value(
            &[PortSpec::color("color", 1.0, 1.0, 1.0), unit("amount", 1.0)],
            number("value"),
            "rikka_npr_skin_reject($context, {color}, {amount})",
        ),
        Function::new(
            "npr.hair_shadow",
            "Screen-space hair shadow",
            "How much front hair shadows this pixel from the light: the hair depth layer is read along the light's screen direction distance metres away with softness-wide taps, shadowing where hair lies 0…reach metres in front; lit = 1 - shadow.",
        )
        .packed(
            &[
                LIGHT,
                PortSpec::number("distance", 0.03).soft(0.0, 0.2),
                PortSpec::number("reach", 0.15).soft(0.0, 1.0),
                PortSpec::number("softness", 0.6).soft(0.0, 1.5),
            ],
            "rikka_npr_hair_shadow($pixel, $context, {light}, {distance}, {reach}, {softness})",
            "RikkaNprHairShadow",
            &[(number("shadow"), ".shadow"), (number("lit"), ".lit")],
        ),
        Function::new(
            "npr.eye_parallax",
            "Eye parallax",
            "uv shifted so the iris seems to sit depth uv units behind the eye's surface, the view ray bent by ior when above 1, scaled by mask.",
        )
        .value(
            &[
                UV,
                PortSpec::number("depth", 0.05).soft(0.0, 0.2),
                PortSpec::number("ior", 1.0).soft(1.0, 1.6),
                unit("mask", 1.0),
            ],
            PortSpec::output("uv", ValueType::Float2),
            "rikka_npr_eye_parallax($pixel, {uv}, {depth}, {ior}, {mask})",
        ),
    ]
}
