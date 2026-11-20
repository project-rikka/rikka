use bytemuck::{Pod, Zeroable};

use crate::gpu::NONE_INDEX;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GpuScene {
    pub previous_view_projection: [f32; 16],
    pub depth: u32,
    pub surface: u32,
    pub figure_count: u32,
    pub enabled: u32,
    pub history: u32,
    pub styled: u32,
    pub stylization: f32,
    pub shade_softness: f32,
    pub shadow_tint: [f32; 3],
    pub ambient: f32,
    pub previous_time: f32,
    pub hair: u32,
    pub face: u32,
    pub brdf: u32,
    pub scatter: u32,
    pub refraction: u32,
    pub ltc_inverse: u32,
    pub ltc_amplitude: u32,
    pub previous_sky_view_projection: [f32; 16],
}

impl Default for GpuScene {
    fn default() -> Self {
        Self {
            previous_view_projection: [0.0; 16],
            depth: NONE_INDEX,
            surface: NONE_INDEX,
            figure_count: 0,
            enabled: 0,
            history: 0,
            styled: 0,
            stylization: 0.0,
            shade_softness: 0.0,
            shadow_tint: [1.0; 3],
            ambient: 0.0,
            previous_time: 0.0,
            hair: NONE_INDEX,
            face: NONE_INDEX,
            brdf: NONE_INDEX,
            scatter: NONE_INDEX,
            refraction: NONE_INDEX,
            ltc_inverse: NONE_INDEX,
            ltc_amplitude: NONE_INDEX,
            previous_sky_view_projection: [0.0; 16],
        }
    }
}

const _: () = assert!(size_of::<GpuScene>() == 208);
