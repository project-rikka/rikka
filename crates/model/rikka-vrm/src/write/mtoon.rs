use serde_json::Value;

use rikka_gltf::Composition;
use rikka_model_core::Model;
use rikka_model_core::{Error, Result};
use rikka_model_core::{Material, OutlineWidth, Shading, Toon};

use super::super::document as raw;

pub(super) fn write(model: &Model, composition: &mut Composition) -> Result<()> {
    for (index, material) in model.materials.iter().enumerate() {
        let Shading::Toon(toon) = material.shading else {
            continue;
        };
        let value = described(&mtoon(material, &toon))?;
        if !composition.set_material_extension(index, raw::MTOON, value) {
            return Err(missing("material"));
        }
    }
    Ok(())
}

pub(super) fn used(model: &Model) -> bool {
    model
        .materials
        .iter()
        .any(|material| matches!(material.shading, Shading::Toon(_)))
}

fn mtoon(material: &Material, toon: &Toon) -> raw::MToon {
    let textures = &material.textures;
    raw::MToon {
        spec_version: raw::SPEC_VERSION.to_owned(),
        transparent_with_z_write: Some(toon.transparent_depth_write),
        render_queue_offset_number: Some(toon.render_order),
        shade_color_factor: Some(toon.shade_color.to_array()),
        shade_multiply_texture: info(textures.shade),
        shading_shift_factor: Some(toon.shading_shift),
        shading_shift_texture: textures
            .shading_shift
            .map(|texture| raw::ShadingShiftTexture {
                index: texture.0,
                tex_coord: 0,
                scale: Some(toon.shading_shift_scale),
            }),
        shading_toony_factor: Some(toon.shading_toony),
        gi_equalization_factor: Some(toon.gi_equalization),
        matcap_factor: Some(toon.matcap_color.to_array()),
        matcap_texture: info(textures.matcap),
        parametric_rim_color_factor: Some(toon.rim_color.to_array()),
        rim_multiply_texture: info(textures.rim),
        rim_lighting_mix_factor: Some(toon.rim_lighting_mix),
        parametric_rim_fresnel_power_factor: Some(toon.rim_fresnel_power),
        parametric_rim_lift_factor: Some(toon.rim_lift),
        outline_width_mode: Some(
            match toon.outline.width {
                OutlineWidth::None => "none",
                OutlineWidth::World(_) => "worldCoordinates",
                OutlineWidth::Screen(_) => "screenCoordinates",
            }
            .to_owned(),
        ),
        outline_width_factor: Some(toon.outline.width.width()),
        outline_width_multiply_texture: info(textures.outline_width),
        outline_color_factor: Some(toon.outline.color.to_array()),
        outline_lighting_mix_factor: Some(toon.outline.lighting_mix),
        uv_animation_mask_texture: info(textures.uv_animation_mask),
        uv_animation_scroll_x_speed_factor: Some(toon.scroll.x),
        uv_animation_scroll_y_speed_factor: Some(toon.scroll.y),
        uv_animation_rotation_speed_factor: Some(toon.spin),
    }
}

fn info(texture: Option<rikka_model_core::TextureId>) -> Option<raw::TextureInfo> {
    texture.map(|texture| raw::TextureInfo {
        index: texture.0,
        tex_coord: 0,
    })
}

fn described(mtoon: &raw::MToon) -> Result<Value> {
    serde_json::to_value(mtoon).map_err(|error| Error::Encode {
        what: "a toon material",
        reason: error.to_string(),
    })
}

fn missing(what: &str) -> Error {
    Error::Invalid(format!("written document is missing {what}"))
}
