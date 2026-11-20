mod atlas;
mod cache;
mod frusta;
mod plan;
pub(crate) mod store;

pub(crate) use atlas::AtlasAllocator;
pub(crate) use cache::ShadowCache;
pub(crate) use plan::{Allocators, Key, Plan, PlanInput, Planned, Traced};
pub(crate) use store::ShadowAtlas;

use glam::{Vec2, Vec3};

use crate::gpu::GpuShadowSlot;

pub(crate) const LARGEST_ATLAS: u32 = 4096;

pub(crate) const LARGEST_FIGURE_ATLAS: u32 = 4096;

pub(crate) const CASCADES: u32 = 4;

pub(crate) const DEFAULT_SHADOW_DISTANCE: f32 = 120.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Bounds {
    pub low: Vec3,
    pub high: Vec3,
}

impl Bounds {
    pub(crate) const EMPTY: Self = Self {
        low: Vec3::splat(f32::INFINITY),
        high: Vec3::splat(f32::NEG_INFINITY),
    };

    pub(crate) fn enclose(&mut self, center: Vec3, radius: f32) {
        self.low = self.low.min(center - Vec3::splat(radius));
        self.high = self.high.max(center + Vec3::splat(radius));
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.low.x > self.high.x
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Atlas {
    Main,
    Figures,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShadowView {
    pub atlas: Atlas,
    pub view_projection: glam::Mat4,
    pub origin: (u32, u32),
    pub tile: u32,
    pub split_depth: f32,
    pub depth_bias: f32,
    pub normal_bias: f32,
    pub figure: u32,
    pub channels: u32,
    pub flags: u32,
    pub near: f32,
    pub projection_scale: f32,
    pub depth_range: f32,
    pub world_texel: f32,
}

impl ShadowView {
    pub(crate) fn to_gpu(self, atlas: u32) -> GpuShadowSlot {
        let scale = self.tile as f32 / atlas as f32;
        GpuShadowSlot {
            view_projection: self.view_projection.to_cols_array(),
            uv_scale: [scale, scale],
            uv_offset: [
                self.origin.0 as f32 / atlas as f32,
                self.origin.1 as f32 / atlas as f32,
            ],
            depth_bias: self.depth_bias,
            normal_bias: self.normal_bias,
            split_depth: self.split_depth,
            texel_size: 1.0 / atlas as f32,
            figure: self.figure,
            channels: self.channels,
            flags: self.flags,
            near: self.near,
            projection_scale: self.projection_scale,
            depth_range: self.depth_range,
            world_texel: self.world_texel,
            _pad: 0,
        }
    }

    pub(crate) fn is_cached(&self) -> bool {
        self.flags & crate::gpu::shadow_flags::CACHED != 0
    }
}

pub(crate) fn tile_viewport(view: &ShadowView) -> (Vec2, Vec2) {
    (
        Vec2::new(view.origin.0 as f32, view.origin.1 as f32),
        Vec2::splat(view.tile as f32),
    )
}
