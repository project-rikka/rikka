use rikka_slang::{ParameterKind, ParameterSchema, Role};

use crate::gpu::{GpuMaterial, NONE_INDEX};

const ALBEDO_ALPHA: u32 = 1 << 0;

pub(crate) fn record(schema: &ParameterSchema, block: u32, alpha_cutoff: f32) -> GpuMaterial {
    let offset = |role: Role| {
        schema
            .with_role(role)
            .map(|parameter| (parameter.offset as u32, &parameter.kind))
    };
    let albedo = offset(Role::Albedo);
    let albedo_alpha = albedo.is_some_and(|(_, kind)| {
        matches!(
            kind,
            ParameterKind::Vec4 | ParameterKind::Color { alpha: true, .. }
        )
    });
    let at = |found: Option<(u32, &ParameterKind)>| found.map_or(NONE_INDEX, |(offset, _)| offset);
    let role = |role: Role| at(offset(role));
    GpuMaterial {
        block,
        alpha_cutoff,
        albedo: at(albedo),
        albedo_texture: role(Role::AlbedoTexture),
        emission: role(Role::Emission),
        emission_texture: role(Role::EmissionTexture),
        roles: match albedo_alpha {
            true => ALBEDO_ALPHA,
            false => 0,
        },
        albedo_weight: role(Role::AlbedoWeight),
        metalness: role(Role::Metalness),
        metalness_texture: role(Role::MetalnessTexture),
        transmission: role(Role::Transmission),
        emission_luminance: role(Role::EmissionLuminance),
    }
}
