use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::graph::groups;

const GROUP: u32 = 8;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct KernelPush {
    pub frame: u64,
    pub target: u32,
    pub pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct PrefilterPush {
    pub frame: u64,
    pub target: u32,
    pub source: u32,
    pub levels: u32,
    pub pad: u32,
}

pub(super) fn covering((width, height): (u32, u32)) -> (u32, u32) {
    groups(vk::Extent2D { width, height }, GROUP)
}
