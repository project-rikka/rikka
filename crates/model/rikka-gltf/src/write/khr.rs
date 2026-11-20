use rikka_model_core::glam::Vec3;
use rikka_model_core::{MaterialTextures, Pbr};

use crate::document::{
    Document, MaterialExtensions, NormalTextureInfo, TextureInfo, TextureInfoExtensions, khr,
};

type Present = fn(&MaterialExtensions) -> bool;

const PRESENT: [(&str, Present); 9] = [
    ("KHR_materials_ior", |extensions| extensions.ior.is_some()),
    ("KHR_materials_specular", |extensions| {
        extensions.specular.is_some()
    }),
    ("KHR_materials_anisotropy", |extensions| {
        extensions.anisotropy.is_some()
    }),
    ("KHR_materials_clearcoat", |extensions| {
        extensions.clearcoat.is_some()
    }),
    ("KHR_materials_sheen", |extensions| {
        extensions.sheen.is_some()
    }),
    ("KHR_materials_transmission", |extensions| {
        extensions.transmission.is_some()
    }),
    ("KHR_materials_volume", |extensions| {
        extensions.volume.is_some()
    }),
    ("KHR_materials_dispersion", |extensions| {
        extensions.dispersion.is_some()
    }),
    ("KHR_materials_iridescence", |extensions| {
        extensions.iridescence.is_some()
    }),
];

pub(crate) fn write(
    extensions: &mut MaterialExtensions,
    pbr: &Pbr,
    textures: &MaterialTextures,
    placement: &TextureInfoExtensions,
) {
    let info = |texture: Option<rikka_model_core::TextureId>| {
        texture.map(|texture| TextureInfo {
            index: texture.0,
            tex_coord: 0,
            extensions: placement.clone(),
        })
    };
    let defaults = Pbr::default();
    if pbr.specular.ior != defaults.specular.ior {
        extensions.ior = Some(khr::Ior {
            ior: Some(pbr.specular.ior),
        });
    }
    if pbr.specular.weight != defaults.specular.weight
        || pbr.specular.color != Vec3::ONE
        || textures.specular.is_some()
        || textures.specular_color.is_some()
    {
        extensions.specular = Some(khr::Specular {
            specular_factor: Some(pbr.specular.weight),
            specular_texture: info(textures.specular),
            specular_color_factor: Some(pbr.specular.color.to_array()),
            specular_color_texture: info(textures.specular_color),
        });
    }
    if pbr.anisotropy.strength > 0.0 {
        extensions.anisotropy = Some(khr::Anisotropy {
            anisotropy_strength: Some(pbr.anisotropy.strength),
            anisotropy_rotation: Some(pbr.anisotropy.rotation),
            anisotropy_texture: info(textures.anisotropy),
        });
    }
    if pbr.coat.weight > 0.0 {
        extensions.clearcoat = Some(khr::Clearcoat {
            clearcoat_factor: Some(pbr.coat.weight),
            clearcoat_texture: info(textures.coat),
            clearcoat_roughness_factor: Some(pbr.coat.roughness),
            clearcoat_roughness_texture: info(textures.coat_roughness),
            clearcoat_normal_texture: info(textures.coat_normal).map(|normal| NormalTextureInfo {
                index: normal.index,
                tex_coord: 0,
                scale: Some(pbr.coat.normal_scale),
                extensions: normal.extensions,
            }),
        });
    }
    if pbr.sheen.weight > 0.0 {
        extensions.sheen = Some(khr::Sheen {
            sheen_color_factor: Some((pbr.sheen.color * pbr.sheen.weight).to_array()),
            sheen_color_texture: info(textures.sheen_color),
            sheen_roughness_factor: Some(pbr.sheen.roughness),
            sheen_roughness_texture: info(textures.sheen_roughness),
        });
    }
    if pbr.transmission.weight > 0.0 {
        extensions.transmission = Some(khr::Transmission {
            transmission_factor: Some(pbr.transmission.weight),
            transmission_texture: info(textures.transmission),
        });
        if !pbr.thin_walled {
            extensions.volume = Some(khr::Volume {
                thickness_factor: Some(pbr.transmission.thickness),
                thickness_texture: info(textures.thickness),
                attenuation_distance: pbr
                    .transmission
                    .attenuation_distance
                    .is_finite()
                    .then_some(pbr.transmission.attenuation_distance),
                attenuation_color: Some(pbr.transmission.attenuation_color.to_array()),
            });
        }
        if pbr.transmission.dispersion > 0.0 {
            extensions.dispersion = Some(khr::Dispersion {
                dispersion: Some(pbr.transmission.dispersion),
            });
        }
    }
    if pbr.iridescence.weight > 0.0 {
        extensions.iridescence = Some(khr::Iridescence {
            iridescence_factor: Some(pbr.iridescence.weight),
            iridescence_texture: info(textures.iridescence),
            iridescence_ior: Some(pbr.iridescence.ior),
            iridescence_thickness_minimum: Some(pbr.iridescence.thickness_min),
            iridescence_thickness_maximum: Some(pbr.iridescence.thickness_max),
            iridescence_thickness_texture: info(textures.iridescence_thickness),
        });
    }
}

pub(crate) fn declare(document: &mut Document) {
    for (name, present) in PRESENT {
        if document
            .materials
            .iter()
            .any(|material| present(&material.extensions))
        {
            document.extensions_used.push(name.to_owned());
        }
    }
}
