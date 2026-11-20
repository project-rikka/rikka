use bytemuck::{Pod, Zeroable};

use crate::gpu::NONE_INDEX;

pub(crate) const TRACED_LIGHTS: usize = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GpuTrace {
    pub scene: [u32; 2],
    pub parts: [u32; 2],
    pub indices16: [u32; 2],
    pub indices32: [u32; 2],
    pub shadow_lights: [u32; TRACED_LIGHTS],
    pub lights: [u32; TRACED_LIGHTS],
    pub visibility_image: u32,
    pub indirect_image: u32,
    pub radiance_image: u32,
    pub frame: u32,
    pub indirect_strength: f32,
    pub reach: f32,
    pub hooks: [u32; 2],
    pub shadow_samples: u32,
    pub _pad: [u32; 3],
}

impl Default for GpuTrace {
    fn default() -> Self {
        Self {
            shadow_lights: [NONE_INDEX; TRACED_LIGHTS],
            lights: [NONE_INDEX; TRACED_LIGHTS],
            visibility_image: NONE_INDEX,
            indirect_image: NONE_INDEX,
            radiance_image: NONE_INDEX,
            ..Zeroable::zeroed()
        }
    }
}

pub(crate) fn split(address: u64) -> [u32; 2] {
    [address as u32, (address >> 32) as u32]
}

const _: () = assert!(size_of::<GpuTrace>() == 112);
