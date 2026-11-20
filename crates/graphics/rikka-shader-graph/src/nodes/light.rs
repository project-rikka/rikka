use crate::library::NodeSpec;
use crate::node::{Input, PortSpec};
use crate::nodes::{Function, LIGHT, NORMAL, VIEW, number, spatial, unit};
use crate::ops::Category;
use crate::types::ValueType;

pub(super) fn specs() -> Vec<NodeSpec> {
    vec![
        Function::new(
            "light.sample",
            "Light sample",
            "The light being added: direction towards it (an area light's representative direction), radiance colour, falloff, irradiance = falloff × clipped cosine at normal (exact for area lights), leading (1 for the key light), area (1 for area lights), angular_radius as the sine of its half-angle and distance in metres (0 for directional lights).",
        )
        .category(Category::Lighting)
        .packed(
            &[NORMAL],
            "rikka_npr_light($light, $context, {normal})",
            "RikkaNprLight",
            &[
                (spatial("direction"), ".direction"),
                (spatial("color"), ".color"),
                (number("attenuation"), ".attenuation"),
                (number("irradiance"), ".irradiance"),
                (number("leading"), ".leading"),
                (number("area"), ".area"),
                (number("angular_radius"), ".angular_radius"),
                (number("distance"), ".distance"),
            ],
        ),
        Function::new(
            "light.shadow",
            "Light shadow",
            "The light's shadow: visibility = map × screen, transmittance tinted by coloured casters, tint their normalized colour, filter the colour coloured casters let through on its own (1 without them), map the filtered shadow map or traced shadow, screen the screen-space shadow, hard one unfiltered tap, blocker the receiver-blocker distance in metres (-1 unmeasured), penumbra width, and faded = visibility fading out for blockers nearer than near metres.",
        )
        .category(Category::Lighting)
        .packed(
            &[PortSpec::number("near", 0.0).soft(0.0, 0.1)],
            "rikka_npr_shadow($shadow, {near})",
            "RikkaNprShadow",
            &[
                (number("visibility"), ".visibility"),
                (spatial("transmittance"), ".transmittance"),
                (spatial("tint"), ".tint"),
                (spatial("filter"), ".filter"),
                (number("map"), ".map"),
                (number("screen"), ".screen"),
                (number("hard"), ".hard"),
                (number("blocker"), ".blocker"),
                (number("penumbra"), ".penumbra"),
                (number("faded"), ".faded"),
            ],
        ),
        Function::new(
            "npr.cast_gate",
            "Cast-shadow gate",
            "lerp(floor, lit, cast): cast = shadow faded in over facing 0…terminator (terminator 0 keeps it whole), or 1 where mask exceeds bypass.",
        )
        .value(
            &[
                unit("lit", 1.0),
                PortSpec::context("shadow", ValueType::Float, Input::Shadow),
                unit("floor", 0.0),
                unit("mask", 0.0),
                unit("bypass", 0.8),
                PortSpec::number("facing", 1.0).range(-1.0, 1.0),
                PortSpec::number("terminator", 0.0).soft(0.0, 1.0),
            ],
            number("value"),
            "rikka_npr_cast_gate({lit}, {shadow}, {floor}, {mask}, {bypass}, {facing}, {terminator})",
        ),
        Function::new(
            "npr.light_scale",
            "Cel light scale",
            "1/π in a looked frame, where lights carry physical irradiance, and 1 otherwise: multiplied into a cel term so a unit light shows the painted colour in plain frames and Lambert-equivalent brightness under a look.",
        )
        .category(Category::Lighting)
        .value(&[], number("value"), "rikka_npr_light_scale($context)"),
        Function::new(
            "light.indirect",
            "Indirect light",
            "Light from the surroundings at normal: diffuse along the bent normal with multi-bounce occlusion coloured by albedo (or the traced bounce), sky the unoccluded sky irradiance, reflection the sky along the dominant direction times specular occlusion, occlusion and screen_shadow the screen terms, bent_normal.",
        )
        .category(Category::Lighting)
        .packed(
            &[
                NORMAL,
                VIEW,
                unit("roughness", 0.5),
                PortSpec::color("albedo", 1.0, 1.0, 1.0),
                unit("occlusion", 1.0),
            ],
            "rikka_npr_indirect($indirect, {normal}, {view}, {roughness}, {albedo}, {occlusion})",
            "RikkaNprIndirect",
            &[
                (spatial("diffuse"), ".diffuse"),
                (spatial("sky"), ".sky"),
                (spatial("reflection"), ".reflection"),
                (number("occlusion"), ".occlusion"),
                (number("screen_shadow"), ".screen_shadow"),
                (spatial("bent_normal"), ".bent_normal"),
            ],
        ),
        Function::new(
            "light.environment",
            "Environment tint",
            "Genshin-style ambient: multiply = max(lowest, mean of the up and down sky, the down sky, main_light); add = min(sky(normal),1) × its luminance × strength, fading out as it brightens; equalized = sky(normal) blended towards the up/down mean by equalization (MToon GI equalization).",
        )
        .category(Category::Lighting)
        .packed(
            &[
                NORMAL,
                PortSpec::color("main_light", 0.0, 0.0, 0.0),
                PortSpec::number("strength", 1.0).soft(0.0, 2.0),
                unit("equalization", 0.0),
                PortSpec::number("lowest", 0.05).soft(0.0, 1.0),
            ],
            "rikka_npr_environment($indirect, {normal}, {main_light}, {strength}, {equalization}, {lowest})",
            "RikkaNprEnvironment",
            &[
                (spatial("multiply"), ".multiply"),
                (spatial("add"), ".add"),
                (spatial("equalized"), ".equalized"),
            ],
        ),
        Function::new(
            "npr.head_light",
            "Head-plane light",
            "light flattened onto the head's horizontal plane: flat direction, forward = F·flat, right = R·flat (right is up × forward, screen right on a face looking into the camera), facing = forward×0.5+0.5, on_right 1 when right > 0, upright 0 when the light is straight above or below.",
        )
        .packed(
            &[LIGHT],
            "rikka_npr_head_light($context, {light})",
            "RikkaNprHeadLight",
            &[
                (spatial("flat"), ".flat"),
                (number("forward"), ".forward"),
                (number("right"), ".right"),
                (number("facing"), ".facing"),
                (number("on_right"), ".on_right"),
                (number("upright"), ".upright"),
            ],
        ),
    ]
}
