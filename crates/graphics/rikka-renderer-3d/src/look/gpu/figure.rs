use bytemuck::{Pod, Zeroable};
use glam::Vec3;

use crate::gpu::NONE_INDEX;
use crate::look::Head;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GpuFigure {
    pub head_center: [f32; 3],
    pub head_radius: f32,
    pub head_forward: [f32; 3],
    pub has_head: u32,
    pub head_up: [f32; 3],
    pub _pad: f32,
    pub shadow: u32,
    pub key_light: u32,
    pub flags: u32,
    pub _pad_flags: u32,
}

impl GpuFigure {
    pub(crate) const NONE: Self = Self {
        head_center: [0.0; 3],
        head_radius: 0.0,
        head_forward: [0.0, 0.0, 1.0],
        has_head: 0,
        head_up: [0.0, 1.0, 0.0],
        _pad: 0.0,
        shadow: NONE_INDEX,
        key_light: NONE_INDEX,
        flags: 0,
        _pad_flags: 0,
    };

    pub(crate) fn from_head(head: Option<Head>) -> Self {
        let Some(head) = head else {
            return Self::NONE;
        };
        Self {
            head_center: head.center.to_array(),
            head_radius: head.radius,
            head_forward: head.forward.normalize_or(Vec3::Z).to_array(),
            has_head: 1,
            head_up: head.up.normalize_or(Vec3::Y).to_array(),
            ..Self::NONE
        }
    }
}

const _: () = assert!(size_of::<GpuFigure>() == 64);
