use rikka_slang::TextureKind;

use crate::library::{Bypass, NodeSpec};
use crate::node::{Input, PortSpec};
use crate::nodes::{
    Function, LIGHT, NORMAL, POSITION, TANGENT, UV, VIEW, number, plane, spatial, texture, unit,
};
use crate::types::{Constant, ValueType};

pub(super) fn specs() -> Vec<NodeSpec> {
    vec![
        Function::new(
            "npr.stepped_specular",
            "Stepped specular",
            "A painted highlight revealed by Blinn-Phong: term = max(N·H, 0.001)^shininess, revealed = step (smoothstep ± softness) of term against offset - threshold, value = revealed × intensity (Genshin offset 1.015, Star Rail 1.03).",
        )
        .packed(
            &[
                NORMAL,
                LIGHT,
                VIEW,
                PortSpec::number("shininess", 32.0).soft(1.0, 256.0),
                unit("threshold", 0.5),
                PortSpec::number("intensity", 1.0).soft(0.0, 4.0),
                PortSpec::number("softness", 0.0).soft(0.0, 0.5),
                PortSpec::number("offset", 1.015).soft(0.9, 1.1),
            ],
            "rikka_npr_stepped({normal}, {light}, {view}, {shininess}, {threshold}, {intensity}, {softness}, {offset})",
            "RikkaNprStepped",
            &[
                (number("value"), ".value"),
                (number("term"), ".term"),
                (number("revealed"), ".revealed"),
            ],
        ),
        Function::new(
            "npr.toon_ggx",
            "Toon GGX",
            "Schlick Fresnel from f0 times the GGX distribution at roughness (perceptual), or times saturate(gain × D) when gain > 0 for a near-step highlight; no cosine or visibility term.",
        )
        .value(
            &[
                NORMAL,
                LIGHT,
                VIEW,
                unit("roughness", 0.3),
                PortSpec::color("f0", 0.04, 0.04, 0.04),
                PortSpec::number("gain", 0.0).soft(0.0, 100.0),
            ],
            spatial("value"),
            "rikka_npr_toon_ggx({normal}, {light}, {view}, {roughness}, {f0}, {gain})",
        ),
        Function::new(
            "npr.matcap_uv",
            "Matcap uv",
            "Where the normal lands on a matcap, ×0.495 + 0.5 with v growing upwards: stable keeps it steady when the camera rolls (MToon), view_normal uses the view-space normal, rolled the view-space normal turned back by the camera roll (UTS); tile scales u, flip turns v top-down.",
        )
        .value(
            &[
                NORMAL,
                VIEW,
                PortSpec::options("mode", &["stable", "view_normal", "rolled"], 0),
                PortSpec::number("tile", 1.0).soft(0.0, 4.0),
                PortSpec::toggle("flip", false),
            ],
            PortSpec::output("uv", ValueType::Float2),
            "rikka_npr_matcap_uv($context, {normal}, {view}, {mode}, {tile}, {flip})",
        ),
        Function::new(
            "npr.matcap",
            "Matcap",
            "Reads a matcap at uv + scroll × time (mip lod) tinted, and blends it into base by strength × mask × its alpha: add, multiply, mix, overlay or screen.",
        )
        .packed(
            &[
                PortSpec::color("base", 0.0, 0.0, 0.0),
                texture("texture", TextureKind::Colour),
                UV,
                PortSpec::color("tint", 1.0, 1.0, 1.0),
                unit("mask", 1.0),
                PortSpec::number("strength", 1.0).soft(0.0, 4.0),
                PortSpec::options("mode", &["add", "multiply", "mix", "overlay", "screen"], 0),
                PortSpec::number("lod", 0.0).soft(0.0, 8.0),
                plane("scroll", 0.0, 0.0),
                PortSpec::context("time", ValueType::Float, Input::Time),
            ],
            "rikka_npr_matcap({base}, {texture}, {uv}, {tint}, {mask}, {strength}, {mode}, {lod}, {scroll}, {time})",
            "RikkaNprMatcap",
            &[
                (spatial("color"), ".color"),
                (spatial("matcap"), ".matcap"),
                (number("alpha"), ".alpha"),
            ],
        ),
        Function::new(
            "npr.metal",
            "Metal matcap",
            "Genshin metal: albedo × lerp(dark, light, saturate(value × brightness)) plus half its highlight (sharp_color where saturate(term) > sharp_edge, else term × specular_color × intensity), the highlight scaled by shadow_specular and the result by shadow_color as shadow goes 0…1.",
        )
        .value(
            &[
                PortSpec::color("albedo", 1.0, 1.0, 1.0),
                unit("value", 0.5),
                PortSpec::color("dark", 0.25, 0.25, 0.3),
                PortSpec::color("light", 1.0, 1.0, 1.0),
                PortSpec::number("brightness", 1.0).soft(0.0, 4.0),
                PortSpec::number("term", 0.0).soft(0.0, 1.0),
                unit("sharp_edge", 0.9),
                PortSpec::color("sharp_color", 1.0, 1.0, 1.0),
                PortSpec::color("specular_color", 1.0, 1.0, 1.0),
                PortSpec::number("intensity", 1.0).soft(0.0, 4.0),
                unit("shadow", 0.0),
                unit("shadow_specular", 0.2),
                PortSpec::color("shadow_color", 1.0, 1.0, 1.0),
            ],
            spatial("value"),
            "rikka_npr_metal({albedo}, {value}, {dark}, {light}, {brightness}, {term}, {sharp_edge}, {sharp_color}, {specular_color}, {intensity}, {shadow}, {shadow_specular}, {shadow_color})",
        ),
        Function::new(
            "npr.packed_matcap",
            "Packed matcap",
            "Wuthering Waves' four-response matcap: lerp(r, g, shine), then towards b by saturate(3·roughness - 1) and towards a by saturate(3·roughness - 2).",
        )
        .value(
            &[
                PortSpec::value("texel", ValueType::Float4, Constant::vec4(0.0, 0.0, 0.0, 0.0)),
                unit("shine", 0.0),
                unit("roughness", 0.0),
            ],
            number("value"),
            "rikka_npr_packed_matcap({texel}, {shine}, {roughness})",
        ),
        Function::new(
            "npr.kajiya_kay",
            "Kajiya-Kay",
            "Two strand highlights along the tangent (made perpendicular to the normal): each lobe is sin(T',H)^exponent × smoothstep(-1, 0, T'·H) with T' = normalize(T + shift × N), times its strength; color = primary + second_color × secondary.",
        )
        .packed(
            &[
                TANGENT,
                NORMAL,
                LIGHT,
                VIEW,
                PortSpec::number("shift", 0.1).soft(-1.0, 1.0),
                PortSpec::number("second_shift", -0.1).soft(-1.0, 1.0),
                PortSpec::number("exponent", 160.0).soft(1.0, 512.0),
                PortSpec::number("second_exponent", 28.0).soft(1.0, 512.0),
                PortSpec::number("strength", 0.6).soft(0.0, 2.0),
                PortSpec::number("second_strength", 0.2).soft(0.0, 2.0),
                PortSpec::color("second_color", 1.0, 1.0, 1.0),
            ],
            "rikka_npr_kajiya({tangent}, {normal}, {light}, {view}, {shift}, {second_shift}, {exponent}, {second_exponent}, {strength}, {second_strength}, {second_color})",
            "RikkaNprKajiya",
            &[
                (spatial("color"), ".color"),
                (number("primary"), ".primary"),
                (number("secondary"), ".secondary"),
            ],
        ),
        Function::new(
            "npr.strand_band",
            "Strand band",
            "A stylised hair band along a painted strand gradient: 1 within width of offset + (T·H) × shift, the gradient jittered by (jitter - 0.5) × jitter_scale, softened over the outer softness share of width.",
        )
        .value(
            &[
                unit("gradient", 0.0),
                unit("jitter", 0.5),
                PortSpec::number("jitter_scale", 0.1).soft(0.0, 1.0),
                TANGENT,
                LIGHT,
                VIEW,
                unit("offset", 0.5),
                PortSpec::number("shift", 0.5).soft(-1.0, 1.0),
                PortSpec::number("width", 0.08).soft(0.0, 0.5),
                unit("softness", 0.5),
            ],
            number("value"),
            "rikka_npr_strand_band({gradient}, {jitter}, {jitter_scale}, {tangent}, {light}, {view}, {offset}, {shift}, {width}, {softness})",
        ),
        Function::new(
            "npr.sphere_band",
            "Head-sphere band",
            "ZZZ hair highlight: the normal bent towards the head sphere's (blend 1 - saturate(falloff × (distance - radius)) in metres), band = saturate((mask - (1 - √HL(L·N) × HL(N·H))) / softness) reveals the painted mask; normal is the bent normal.",
        )
        .packed(
            &[
                POSITION,
                NORMAL,
                LIGHT,
                VIEW,
                unit("mask", 0.0),
                PortSpec::number("softness", 0.1).soft(0.0, 1.0),
                PortSpec::number("falloff", 20.0).soft(0.0, 100.0),
            ],
            "rikka_npr_sphere_band($context, {position}, {normal}, {light}, {view}, {mask}, {softness}, {falloff})",
            "RikkaNprSphereBand",
            &[(number("band"), ".band"), (spatial("normal"), ".normal")],
        ),
        Function::new(
            "npr.mask_band",
            "Mask-threshold band",
            "Wuthering Waves hair: with S = saturate(N·H), S × strength × (max(mask, 1 - S) - (1 - S)) / S reveals the painted mask where it exceeds 1 - N·H.",
        )
        .gated("strength", Bypass::Constant(Constant::scalar(0.0)))
        .value(
            &[
                unit("mask", 0.0),
                NORMAL,
                LIGHT,
                VIEW,
                PortSpec::number("strength", 1.0).soft(0.0, 4.0),
            ],
            number("value"),
            "rikka_npr_mask_band({mask}, {normal}, {light}, {view}, {strength})",
        ),
        Function::new(
            "npr.ring_uv",
            "Angel ring uv",
            "UTS angel ring coordinates: u from the view-space normal's x (pulled towards the view axis by offset_u), v blended from uv.y towards the view-space normal's y by offset_v.",
        )
        .value(
            &[NORMAL, UV, unit("offset_u", 0.0), unit("offset_v", 0.0)],
            PortSpec::output("uv", ValueType::Float2),
            "rikka_npr_ring_uv($context, {normal}, {uv}, {offset_u}, {offset_v})",
        ),
        Function::new(
            "npr.stocking",
            "Stocking",
            "Stocking fabric over skin, by mode: denier mixes skin_color under albedo × color by lerp(denier, 1, (1 - N·V)^power) × range.x; hsr is Star Rail's dark edges plus facing sheen from range r/g/b; genshin brightens albedo by color where (N·V')^power faces the view shifted up by shift and darkens by dark_color where (N·V')^sheen_power falls off; wuwa mixes albedo × color to albedo × dark_color between N·V = denier and dark_width.",
        )
        .packed(
            &[
                PortSpec::options("mode", &["denier", "hsr", "genshin", "wuwa"], 0),
                PortSpec::color("albedo", 1.0, 1.0, 1.0),
                NORMAL,
                VIEW,
                PortSpec::vector("range", 1.0, 1.0, 1.0),
                PortSpec::color("color", 1.0, 1.0, 1.0),
                PortSpec::color("skin_color", 0.98, 0.8, 0.72),
                PortSpec::color("dark_color", 0.5, 0.45, 0.45),
                unit("denier", 0.35),
                PortSpec::number("power", 2.0).soft(0.0, 16.0),
                PortSpec::number("sheen_power", 1.0).soft(0.0, 16.0),
                PortSpec::number("dark_width", 0.5).soft(0.0, 4.0),
                unit("roughness", 0.0),
                PortSpec::number("shift", 0.0).soft(-1.0, 1.0),
            ],
            "rikka_npr_stocking({mode}, {albedo}, {normal}, {view}, {range}, {color}, {skin_color}, {dark_color}, {denier}, {power}, {sheen_power}, {dark_width}, {roughness}, {shift})",
            "RikkaNprStocking",
            &[(spatial("color"), ".color"), (number("density"), ".density")],
        ),
    ]
}
