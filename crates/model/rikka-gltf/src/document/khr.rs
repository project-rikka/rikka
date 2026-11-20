use serde::{Deserialize, Serialize};

use super::{NormalTextureInfo, TextureInfo};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Clearcoat {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) clearcoat_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) clearcoat_texture: Option<TextureInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) clearcoat_roughness_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) clearcoat_roughness_texture: Option<TextureInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) clearcoat_normal_texture: Option<NormalTextureInfo>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Sheen {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sheen_color_factor: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sheen_color_texture: Option<TextureInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sheen_roughness_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sheen_roughness_texture: Option<TextureInfo>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Transmission {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) transmission_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) transmission_texture: Option<TextureInfo>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Volume {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) thickness_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) thickness_texture: Option<TextureInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) attenuation_distance: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) attenuation_color: Option<[f32; 3]>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct Ior {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) ior: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Specular {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) specular_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) specular_texture: Option<TextureInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) specular_color_factor: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) specular_color_texture: Option<TextureInfo>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Iridescence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) iridescence_factor: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) iridescence_texture: Option<TextureInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) iridescence_ior: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) iridescence_thickness_minimum: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) iridescence_thickness_maximum: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) iridescence_thickness_texture: Option<TextureInfo>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Anisotropy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) anisotropy_strength: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) anisotropy_rotation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) anisotropy_texture: Option<TextureInfo>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct Dispersion {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) dispersion: Option<f32>,
}
