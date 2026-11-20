use rikka_model_core::glam::{Vec2, Vec3};

use rikka_gltf::TextureMap;
use rikka_model_core::ImportCtx;
use rikka_model_core::Warning;
use rikka_model_core::{Material, Outline, OutlineWidth, Shading, Toon, ToonSurface, UvTransform};

use super::super::document as raw;

pub(super) fn apply(
    material: &mut Material,
    described: &raw::MToon,
    texture_map: &TextureMap,
    ctx: &mut ImportCtx<'_>,
) {
    if !described.spec_version.starts_with("1.") {
        ctx.warn(Warning::Unsupported(format!(
            "material \"{}\": MToon {} read as 1.0",
            material.name, described.spec_version
        )));
    }

    let textures = &mut material.textures;
    textures.shade = described
        .shade_multiply_texture
        .as_ref()
        .and_then(|info| texture_map.texture(info.index));
    textures.shading_shift = described
        .shading_shift_texture
        .as_ref()
        .and_then(|info| texture_map.texture(info.index));
    textures.matcap = described
        .matcap_texture
        .as_ref()
        .and_then(|info| texture_map.texture(info.index));
    textures.rim = described
        .rim_multiply_texture
        .as_ref()
        .and_then(|info| texture_map.texture(info.index));
    textures.outline_width = described
        .outline_width_multiply_texture
        .as_ref()
        .and_then(|info| texture_map.texture(info.index));
    textures.uv_animation_mask = described
        .uv_animation_mask_texture
        .as_ref()
        .and_then(|info| texture_map.texture(info.index));

    let normal_scale = match material.shading {
        Shading::Pbr(pbr) => pbr.normal_scale,
        Shading::Toon(toon) => toon.normal_scale,
    };
    let width = described.outline_width_factor.unwrap_or(0.0);
    material.shading = Shading::Toon(Toon {
        shade_color: Vec3::from_array(described.shade_color_factor.unwrap_or([1.0; 3])),
        shading_shift: described.shading_shift_factor.unwrap_or(0.0),
        shading_shift_scale: described
            .shading_shift_texture
            .as_ref()
            .and_then(|info| info.scale)
            .unwrap_or(1.0),
        shading_toony: described.shading_toony_factor.unwrap_or(0.9),
        gi_equalization: described.gi_equalization_factor.unwrap_or(0.9),
        normal_scale,
        matcap_color: Vec3::from_array(described.matcap_factor.unwrap_or([1.0; 3])),
        rim_color: Vec3::from_array(described.parametric_rim_color_factor.unwrap_or([0.0; 3])),
        rim_fresnel_power: described.parametric_rim_fresnel_power_factor.unwrap_or(5.0),
        rim_lift: described.parametric_rim_lift_factor.unwrap_or(0.0),
        rim_lighting_mix: described.rim_lighting_mix_factor.unwrap_or(1.0),
        outline: Outline {
            width: match described.outline_width_mode.as_deref() {
                Some("worldCoordinates") => OutlineWidth::World(width),
                Some("screenCoordinates") => OutlineWidth::Screen(width),
                _ => OutlineWidth::None,
            },
            color: Vec3::from_array(described.outline_color_factor.unwrap_or([0.0; 3])),
            lighting_mix: described.outline_lighting_mix_factor.unwrap_or(1.0),
        },
        scroll: Vec2::new(
            described.uv_animation_scroll_x_speed_factor.unwrap_or(0.0),
            described.uv_animation_scroll_y_speed_factor.unwrap_or(0.0),
        ),
        spin: described.uv_animation_rotation_speed_factor.unwrap_or(0.0),
        transparent_depth_write: described.transparent_with_z_write.unwrap_or(false),
        render_order: described.render_queue_offset_number.unwrap_or(0),
        surface: ToonSurface::classify(&material.name, None),
        ..Toon::default()
    });
}

pub(super) fn legacy_uv_transform(offset: [f32; 2], scale: [f32; 2]) -> UvTransform {
    UvTransform {
        offset: Vec2::new(offset[0], 1.0 - scale[1] - offset[1]),
        scale: Vec2::from_array(scale),
        rotation: 0.0,
    }
}
