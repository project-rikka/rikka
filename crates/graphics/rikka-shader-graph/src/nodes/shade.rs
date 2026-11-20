use rikka_slang::TextureKind;

use crate::library::{Bypass, NodeSpec};
use crate::node::{Hint, Input, PortSpec};
use crate::nodes::{
    CHANNELS, Function, LIGHT, NORMAL, UV, VIEW, dynamic, integer, number, plane, spatial, texture,
    unit,
};
use crate::ops::Category;
use crate::types::ValueType;

pub(super) fn specs() -> Vec<NodeSpec> {
    let pick = |name: &'static str| PortSpec::dynamic(name, 0.0);
    vec![
        Function::new(
            "npr.wrap",
            "Wrapped light",
            "(N·L + wrap) / (1 + wrap) for the light direction: wrap 1 is half Lambert (0…1), wrap 0 plain N·L (-1…1).",
        )
        .value(
            &[NORMAL, LIGHT, unit("wrap", 1.0)],
            number("value"),
            "rikka_npr_wrap({normal}, {light}, {wrap})",
        ),
        Function::new(
            "npr.threshold",
            "Threshold bias",
            "Biases a facing value (N·L, -1…1) by a painted bias (0.5 neutral): signed_square (Genshin) gives (HL(facing) + (2x|x|+0.5)·occlusion)/2 with x = bias-0.5, linear (Star Rail) min(1, 2·HL(facing)·bias·occlusion), additive (MToon, ZZZ) facing + shift + bias·occlusion·scale; the first two force 1 above lit_above and 0 below shadow_below.",
        )
        .packed(
            &[
                PortSpec::number("facing", 0.0).range(-1.0, 1.0),
                unit("bias", 0.5),
                PortSpec::options("mode", &["signed_square", "linear", "additive"], 0),
                unit("occlusion", 1.0),
                PortSpec::number("shift", 0.0).soft(-1.0, 1.0),
                PortSpec::number("scale", 1.0).soft(0.0, 2.0),
                unit("lit_above", 0.95),
                unit("shadow_below", 0.05),
            ],
            "rikka_npr_threshold({facing}, {bias}, {mode}, {occlusion}, {shift}, {scale}, {lit_above}, {shadow_below})",
            "RikkaNprThreshold",
            &[
                (number("value"), ".value"),
                (number("force_lit"), ".force_lit"),
                (number("force_shadow"), ".force_shadow"),
            ],
        ),
        Function::new(
            "npr.toon_step",
            "Toon step",
            "A cel edge from 0 below edge to 1 above it: smooth and linear over a window feather wide (0 is a hard step), mtoon linear over edge ± (1 - toony), power 1 - ((edge - value)/feather)^softness below the edge (Genshin's ramp-free transition).",
        )
        .value(
            &[
                PortSpec::number("value", 0.5),
                PortSpec::number("edge", 0.5).soft(-1.0, 1.0),
                PortSpec::number("feather", 0.05).soft(0.0, 1.0),
                PortSpec::options("mode", &["smooth", "linear", "mtoon", "power"], 0),
                unit("toony", 0.9),
                PortSpec::number("softness", 1.0).soft(0.1, 4.0),
            ],
            number("value"),
            "rikka_npr_toon_step({value}, {edge}, {feather}, {mode}, {toony}, {softness})",
        ),
        Function::new(
            "npr.bands",
            "Multi band",
            "Three tones from one value: lit above high, shallow between low and high, deep below low, each edge feathered over smoothness; upper_edge and lower_edge peak at 1 on each boundary (for coloured terminator fringes); color sums the band colours by weight.",
        )
        .packed(
            &[
                PortSpec::number("value", 0.0).soft(-3.0, 3.0),
                PortSpec::number("low", -0.333).soft(-3.0, 3.0),
                PortSpec::number("high", 0.333).soft(-3.0, 3.0),
                PortSpec::number("smoothness", 0.1).soft(0.0, 1.0),
                PortSpec::color("lit_color", 1.0, 1.0, 1.0),
                PortSpec::color("shallow_color", 0.8, 0.7, 0.75),
                PortSpec::color("deep_color", 0.55, 0.5, 0.6),
            ],
            "rikka_npr_bands({value}, {low}, {high}, {smoothness}, {lit_color}, {shallow_color}, {deep_color})",
            "RikkaNprBands",
            &[
                (number("lit"), ".lit"),
                (number("shallow"), ".shallow"),
                (number("deep"), ".deep"),
                (number("upper_edge"), ".upper_edge"),
                (number("lower_edge"), ".lower_edge"),
                (spatial("color"), ".color"),
            ],
        ),
        Function::new(
            "npr.ramp",
            "Ramp atlas",
            "Reads one row of a ramp atlas at u + u_bias (mip 0, u clamped inside the texture): row index × group + row_in_group + offset, of rows equal rows counted from the top (or bottom), sampled at the row's centre; blend > 0 mixes in the row second_offset further on (day/night, warm/cool).",
        )
        .packed(
            &[
                texture("texture", TextureKind::Ramp),
                unit("u", 0.5),
                integer("row", 0.0),
                integer("rows", 1.0),
                integer("group", 1.0),
                integer("row_in_group", 0.0),
                integer("offset", 0.0),
                PortSpec::options("origin", &["top", "bottom"], 0),
                PortSpec::number("u_bias", 0.0).soft(-0.1, 0.1),
                integer("second_offset", 0.0),
                unit("blend", 0.0),
            ],
            "rikka_npr_ramp({texture}, {u}, {row}, {rows}, {group}, {row_in_group}, {offset}, {origin}, {u_bias}, {second_offset}, {blend})",
            "float4",
            &[(spatial("rgb"), ".rgb"), (number("a"), ".a")],
        ),
        Function::new(
            "npr.vertical_ramp",
            "MMD toon ramp",
            "Reads an MMD toon texture down its middle column: facing 1 (lit) at the top, -1 at the bottom; top is the lit end for dividing out.",
        )
        .packed(
            &[
                texture("texture", TextureKind::Ramp),
                PortSpec::number("facing", 0.0).range(-1.0, 1.0),
            ],
            "rikka_npr_vertical_ramp({texture}, {facing})",
            "RikkaNprVerticalRamp",
            &[(spatial("color"), ".color"), (spatial("top"), ".top")],
        ),
        Function::new(
            "npr.material_id",
            "Material ID",
            "A zero-based material index from a painted value: genshin bins (0.0→0, 1.0→1, 0.5→2, 0.3→3, 0.7→4, i.e. Genshin id - 1), floor floor(count·v) (Star Rail), reverse count-1-floor(count·v) (ZZZ, count 5), nearest round(v·(count-1)).",
        )
        .value(
            &[
                unit("value", 0.0),
                PortSpec::options("mode", &["genshin", "floor", "reverse", "nearest"], 0),
                integer("count", 8.0),
            ],
            number("index"),
            "rikka_npr_material_id({value}, {mode}, {count})",
        ),
        Function::new(
            "npr.per_id",
            "Per-ID table",
            "Picks v0…v7 by the rounded index (above 7 picks v7); the entries take any width, so one node holds a colour or value per material ID.",
        )
        .value(
            &[
                integer("index", 0.0),
                pick("v0"),
                pick("v1"),
                pick("v2"),
                pick("v3"),
                pick("v4"),
                pick("v5"),
                pick("v6"),
                pick("v7"),
            ],
            dynamic("value"),
            "(({index}) < 0.5 ? {v0} : ({index}) < 1.5 ? {v1} : ({index}) < 2.5 ? {v2} : ({index}) < 3.5 ? {v3} : ({index}) < 4.5 ? {v4} : ({index}) < 5.5 ? {v5} : ({index}) < 6.5 ? {v6} : {v7})",
        ),
        Function::new(
            "npr.param_lut",
            "Parameter table",
            "One texel of a per-material parameter texture (Star Rail's 8×8 LUT: column = material ID, row = parameter), read at the texel centre from the top, mip 0.",
        )
        .value(
            &[
                texture("texture", TextureKind::Data),
                integer("column", 0.0),
                integer("row", 0.0),
                integer("columns", 8.0),
                integer("rows", 8.0),
            ],
            PortSpec::output("value", ValueType::Float4),
            "rikka_npr_param_lut({texture}, {column}, {row}, {columns}, {rows})",
        ),
        Function::new(
            "npr.shade_mix",
            "Shadow colour mix",
            "Colours the shadow side: shadow_side albedo × lerp(shade, 1, lit) (Genshin, lit side pure albedo), full_ramp albedo × shade (Star Rail, the ramp carries both sides), shade_color lerp(shade, albedo, lit) (MToon shade colour).",
        )
        .value(
            &[
                PortSpec::color("albedo", 1.0, 1.0, 1.0),
                unit("lit", 1.0),
                PortSpec::color("shade", 0.75, 0.7, 0.8),
                PortSpec::options("mode", &["shadow_side", "full_ramp", "shade_color"], 0),
            ],
            spatial("value"),
            "rikka_npr_shade_mix({albedo}, {lit}, {shade}, {mode})",
        ),
        Function::new(
            "npr.skin_levels",
            "Skin level adjust",
            "Star Rail's character levels: below shadow_area 0.9 the shadow branch saturate((ramp - middle)/(middle - low)·0.5 + 0.5) × shadow colour, else the light branch saturate((ramp - middle)/(high - middle)·0.5 + 0.5) × light colour, with skin colours where skin is on.",
        )
        .value(
            &[
                PortSpec::color("ramp", 0.5, 0.5, 0.5),
                unit("shadow_area", 1.0),
                PortSpec::toggle("skin", false),
                unit("low", 0.2),
                unit("middle", 0.5),
                unit("high", 0.8),
                PortSpec::color("skin_light", 1.0, 1.0, 1.0),
                PortSpec::color("skin_shadow", 0.9, 0.7, 0.7),
                PortSpec::color("light", 1.0, 1.0, 1.0),
                PortSpec::color("shadow", 0.8, 0.8, 0.9),
            ],
            spatial("value"),
            "rikka_npr_skin_levels({ramp}, {shadow_area}, {skin}, {low}, {middle}, {high}, {skin_light}, {skin_shadow}, {light}, {shadow})",
        ),
        Function::new(
            "npr.brush",
            "Brush threshold jitter",
            "value + (brush.r - 0.5) × amount, the brush texture read at uv × scale turned by rotation radians (mip 0): painterly, stroke-shaped cel edges.",
        )
        .gated("amount", Bypass::Input("value"))
        .value(
            &[
                PortSpec::number("value", 0.0),
                texture("texture", TextureKind::Data),
                UV,
                PortSpec::number("scale", 1.0).soft(0.1, 16.0),
                PortSpec::number("amount", 0.1).soft(0.0, 1.0),
                PortSpec::number("rotation", 0.0).hint(Hint::Angle),
            ],
            number("value"),
            "rikka_npr_brush({value}, {texture}, {uv}, {scale}, {amount}, {rotation})",
        ),
        Function::new(
            "npr.emission",
            "Emission and bloom weight",
            "emission = albedo × color × strength where mask exceeds threshold; boost = 1 + bloom_strength × bloom_color, a multiplier that lifts chosen parts above the bloom threshold (Star Rail).",
        )
        .packed(
            &[
                PortSpec::color("albedo", 1.0, 1.0, 1.0),
                unit("mask", 1.0),
                unit("threshold", 0.5),
                PortSpec::color("color", 1.0, 1.0, 1.0),
                PortSpec::number("strength", 1.0).soft(0.0, 16.0),
                PortSpec::color("bloom_color", 1.0, 1.0, 1.0),
                PortSpec::number("bloom_strength", 0.0).soft(0.0, 4.0),
            ],
            "rikka_npr_emission({albedo}, {mask}, {threshold}, {color}, {strength}, {bloom_color}, {bloom_strength})",
            "RikkaNprEmission",
            &[(spatial("emission"), ".emission"), (spatial("boost"), ".boost")],
        ),
        Function::new(
            "npr.expression",
            "Expression masks",
            "Tints color by each mask channel in turn: c = lerp(c, c × channel colour, mask × strength) for red, green and blue (blush, shyness, expression shadow).",
        )
        .gated("strength", Bypass::Input("color"))
        .value(
            &[
                PortSpec::color("color", 1.0, 1.0, 1.0),
                PortSpec::vector("mask", 0.0, 0.0, 0.0),
                PortSpec::color("red", 1.0, 0.7, 0.7),
                PortSpec::color("green", 1.0, 0.8, 0.8),
                PortSpec::color("blue", 0.7, 0.7, 0.8),
                PortSpec::vector("strength", 1.0, 1.0, 1.0),
            ],
            spatial("value"),
            "rikka_npr_expression({color}, {mask}, {red}, {green}, {blue}, {strength})",
        ),
        Function::new(
            "npr.sdf_line",
            "SDF inner line",
            "Genshin's distance-aware inner lines: threshold = min(distance_scale × distance + thickness, largest, 0.99), line = step (smoothstep ± smoothness beyond far metres) of sdf against it, albedo darkened towards color × albedo by line × strength.",
        )
        .packed(
            &[
                unit("sdf", 0.0),
                PortSpec::context("distance", ValueType::Float, Input::ViewDepth),
                unit("thickness", 0.1),
                PortSpec::number("distance_scale", 0.0).soft(0.0, 0.2),
                unit("largest", 0.99),
                PortSpec::number("far", 0.0).soft(0.0, 20.0),
                PortSpec::number("smoothness", 0.05).soft(0.0, 0.5),
                PortSpec::color("color", 0.3, 0.25, 0.25),
                unit("strength", 1.0),
                PortSpec::color("albedo", 1.0, 1.0, 1.0),
            ],
            "rikka_npr_sdf_line({sdf}, {distance}, {thickness}, {distance_scale}, {largest}, {far}, {smoothness}, {color}, {strength}, {albedo})",
            "RikkaNprLine",
            &[(spatial("color"), ".color"), (number("line"), ".line")],
        ),
        Function::new(
            "npr.channel",
            "Channel pick",
            "One channel of value picked by an option (none gives fallback), optionally inverted; for painted masks whose channel varies by asset.",
        )
        .value(
            &[
                PortSpec::value(
                    "value",
                    ValueType::Float4,
                    crate::types::Constant::vec4(0.0, 0.0, 0.0, 0.0),
                ),
                PortSpec::options("channel", CHANNELS, 1),
                PortSpec::toggle("invert", false),
                PortSpec::number("fallback", 1.0),
            ],
            number("value"),
            "rikka_npr_pick({value}, {channel}, {fallback}, {invert})",
        ),
        Function::new(
            "npr.scatter_bands",
            "Skin scatter bands",
            "Warm bands hugging the cel edge: with b the signed distance from edge (facing - edge, over 1 - edge above and 1 + edge below), color × (1 - |b|/width)² plus deep_color × deep_strength × (1 - |b|/deep_width)², weighted lit_side on the lit side rising to 1 deep in shadow.",
        )
        .value(
            &[
                PortSpec::number("facing", 0.0).range(-1.0, 1.0),
                PortSpec::number("edge", 0.0).range(-1.0, 1.0),
                PortSpec::number("width", 0.25).soft(0.02, 1.0),
                PortSpec::number("deep_width", 0.7).soft(0.02, 1.5),
                PortSpec::number("deep_strength", 0.4).soft(0.0, 2.0),
                unit("lit_side", 0.45),
                PortSpec::color("color", 1.0, 0.45, 0.3),
                PortSpec::color("deep_color", 0.8, 0.14, 0.08),
            ],
            spatial("value"),
            "rikka_npr_scatter_bands({facing}, {edge}, {width}, {deep_width}, {deep_strength}, {lit_side}, {color}, {deep_color})",
        ),
        Function::new(
            "vector.uv_transform",
            "UV transform",
            "uv turned about the centre by spin × time × mask radians and moved by scroll × time × mask (MToon uv animation), then scaled, rotated by rotation and offset as KHR_texture_transform does.",
        )
        .category(Category::Uv)
        .value(
            &[
                UV,
                plane("offset", 0.0, 0.0),
                plane("scale", 1.0, 1.0),
                PortSpec::number("rotation", 0.0).hint(Hint::Angle),
                plane("scroll", 0.0, 0.0),
                PortSpec::number("spin", 0.0).soft(-4.0, 4.0),
                unit("mask", 1.0),
                PortSpec::context("time", ValueType::Float, Input::Time),
            ],
            PortSpec::output("uv", ValueType::Float2),
            "rikka_npr_uv_transform({uv}, {offset}, {scale}, {rotation}, {scroll}, {spin}, {mask}, {time})",
        ),
        Function::new(
            "npr.wet",
            "Wet surface",
            "The mesh's wetness at this pixel for porosity (0 sealed, 1 soaking): albedo darkened and saturated as it soaks, film the standing water film 0…1, film_normal the normal flattened towards the geometric one under the film, film_view the share of the view the film reflects (its Fresnel sheen times film).",
        )
        .packed(
            &[
                PortSpec::color("albedo", 1.0, 1.0, 1.0),
                unit("porosity", 0.85),
                NORMAL,
            ],
            "rikka_npr_wet($pixel, {albedo}, {porosity}, {normal})",
            "RikkaNprWet",
            &[
                (spatial("albedo"), ".albedo"),
                (number("film"), ".film"),
                (spatial("film_normal"), ".film_normal"),
                (number("film_view"), ".film_view"),
            ],
        ),
        Function::new(
            "npr.wet_glint",
            "Wet glint",
            "A crisp highlight of the light on a water film: film × the film's lit side × a narrow glint around N·H on film_normal; multiply by the light's colour and shadow.",
        )
        .value(
            &[
                PortSpec::vector("film_normal", 0.0, 0.0, 1.0),
                unit("film", 0.0),
                LIGHT,
                VIEW,
            ],
            number("value"),
            "rikka_npr_wet_glint({film_normal}, {film}, {light}, {view})",
        ),
        Function::new(
            "npr.wet_film",
            "Wet film reflection",
            "The surroundings on a water film: sheen = film × a soft sky sheen (multiply by the ambient light), reflection the sky mirrored about film_normal at the film's roughness (weight it by film_view).",
        )
        .packed(
            &[
                PortSpec::vector("film_normal", 0.0, 0.0, 1.0),
                unit("film", 0.0),
                VIEW,
            ],
            "rikka_npr_wet_film($indirect, {film_normal}, {film}, {view})",
            "RikkaNprWetFilm",
            &[(number("sheen"), ".sheen"), (spatial("reflection"), ".reflection")],
        ),
    ]
}
