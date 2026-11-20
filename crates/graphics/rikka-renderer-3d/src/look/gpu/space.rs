use bytemuck::{Pod, Zeroable};
use glam::Vec3;

use crate::gpu::NONE_INDEX;
use crate::look::{Look, ParticleKind};

const PARTICLES_PER_AMOUNT: f32 = 16_000.0;
const MAX_PARTICLES: u32 = 65_536;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GpuSpace {
    pub occlusion_image: u32,
    pub occlusion_strength: f32,
    pub occlusion_radius: f32,
    pub reflection_strength: f32,
    pub roughness_limit: f32,
    pub lines_strength: f32,
    pub lines_width: f32,
    pub lines_depth: f32,
    pub lines_crease: f32,
    pub lines_environment: f32,
    pub lines_color: [f32; 3],
    pub particle_count: u32,
    pub particle_kind: u32,
    pub particle_seed: u32,
    pub particle_size: f32,
    pub particle_glow: f32,
    pub particle_reach: f32,
    pub particle_focus: f32,
    pub particle_color: [f32; 3],
    pub particle_wind: [f32; 3],
    pub screen_shadow_image: u32,
    pub screen_shadow_strength: f32,
    pub screen_shadow_thickness: f32,
    pub _pad: [u32; 3],
}

impl Default for GpuSpace {
    fn default() -> Self {
        Self {
            occlusion_image: NONE_INDEX,
            screen_shadow_image: NONE_INDEX,
            ..Zeroable::zeroed()
        }
    }
}

const _: () = assert!(size_of::<GpuSpace>() == 128);

impl GpuSpace {
    pub(crate) fn from_look(look: &Look) -> Self {
        let lines = &look.lines;
        let particles = &look.particles;
        Self {
            occlusion_strength: look.occlusion.strength.max(0.0),
            occlusion_radius: look.occlusion.radius.max(0.01),
            screen_shadow_strength: look.shadows.screen.strength.max(0.0),
            screen_shadow_thickness: look.shadows.screen.thickness.max(1e-4),
            reflection_strength: look.reflections.strength.max(0.0),
            roughness_limit: look.reflections.roughness_limit.clamp(0.0, 1.0),
            lines_strength: lines.strength.max(0.0),
            lines_width: lines.width.max(0.0),
            lines_depth: lines.depth.max(0.0),
            lines_crease: lines.crease.max(0.0),
            lines_environment: lines.environment.max(0.0),
            lines_color: lines.color.max(Vec3::ZERO).to_array(),
            particle_count: Self::particle_count(particles.amount),
            particle_kind: match particles.kind {
                ParticleKind::Motes => 0,
                ParticleKind::Petals => 1,
                ParticleKind::Sparkles => 2,
                ParticleKind::Snow => 3,
            },
            particle_seed: particles.seed,
            particle_size: particles.size.max(0.0),
            particle_glow: particles.glow.max(0.0),
            particle_reach: particles.reach.max(0.1),
            particle_focus: particles.focus.max(0.0),
            particle_color: particles.color.max(Vec3::ZERO).to_array(),
            particle_wind: particles.wind.to_array(),
            ..Self::default()
        }
    }

    pub(crate) fn screen_shadows(&self) -> bool {
        self.screen_shadow_strength > 0.0
    }

    pub(crate) fn particle_count(amount: f32) -> u32 {
        match amount.is_finite() && amount > 0.0 {
            true => ((amount * PARTICLES_PER_AMOUNT).round() as u32).min(MAX_PARTICLES),
            false => 0,
        }
    }
}
