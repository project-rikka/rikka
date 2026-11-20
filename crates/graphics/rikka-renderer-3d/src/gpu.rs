use bytemuck::{Pod, Zeroable};

pub(crate) const CLUSTER_TILE: u32 = 16;

pub(crate) const CLUSTER_SLICES: u32 = 24;

pub(crate) const NONE_INDEX: u32 = u32::MAX;

pub(crate) const UNMAPPED_SHADOW: u32 = 0x8000_0000;

pub(crate) mod instance_flags {
    pub(crate) const CASTS_SHADOW: u32 = 1 << 0;
    pub(crate) const RECEIVES_SHADOW: u32 = 1 << 1;
    pub(crate) const PICKABLE: u32 = 1 << 2;
    pub(crate) const SELECTED: u32 = 1 << 3;
    pub(crate) const NO_OCCLUSION: u32 = 1 << 4;
    pub(crate) const TRANSPARENT: u32 = 1 << 5;
    pub(crate) const DOUBLE_SIDED: u32 = 1 << 6;
    pub(crate) const DIRECT: u32 = 1 << 7;
    pub(crate) const MASKED: u32 = 1 << 8;
    pub(crate) const INDEX16: u32 = 1 << 9;
    pub(crate) const DEFORMED: u32 = 1 << 10;
    pub(crate) const FACIAL: u32 = 1 << 11;
}

pub(crate) mod light_kinds {
    pub(crate) const DIRECTIONAL: u32 = 0;
    pub(crate) const POINT: u32 = 1;
    pub(crate) const SPOT: u32 = 2;
    pub(crate) const RECT: u32 = 3;
    pub(crate) const DISC: u32 = 4;
    pub(crate) const SPHERE: u32 = 5;
    pub(crate) const TUBE: u32 = 6;
}

pub(crate) mod light_flags {
    pub(crate) const LEADING: u32 = 1 << 0;
    pub(crate) const TWO_SIDED: u32 = 1 << 1;
    pub(crate) const KEY: u32 = 1 << 2;
}

pub(crate) mod shadow_flags {
    pub(crate) const PERSPECTIVE: u32 = 1 << 0;
    pub(crate) const FIGURE: u32 = 1 << 1;
    pub(crate) const WITHOUT_MAPPED_FIGURES: u32 = 1 << 2;
    pub(crate) const CACHED: u32 = 1 << 3;
    pub(crate) const TINTED: u32 = 1 << 4;
    pub(crate) const DETAILED: u32 = 1 << 5;
}

pub(crate) mod figure_flags {
    pub(crate) const MAPPED: u32 = 1 << 0;
    pub(crate) const TURNED_KEY: u32 = 1 << 1;
}

pub(crate) fn element_address<T>(base: u64, index: u32) -> u64 {
    base + u64::from(index) * size_of::<T>() as u64
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuView {
    pub view: [f32; 16],
    pub projection: [f32; 16],
    pub view_projection: [f32; 16],
    pub inverse_view_projection: [f32; 16],
    pub inverse_projection: [f32; 16],
    pub frustum: [[f32; 4]; 6],
    pub camera_position: [f32; 3],
    pub near: f32,
    pub camera_forward: [f32; 3],
    pub viewport: [f32; 2],
    pub inverse_viewport: [f32; 2],
    pub cluster_counts: [u32; 3],
    pub cluster_tile: u32,
    pub cluster_depth_scale: f32,
    pub cluster_depth_bias: f32,
    pub light_count: u32,
    pub directional_count: u32,
    pub instance_count: u32,
    pub hzb_size: [f32; 2],
    pub hzb_levels: u32,
    pub exposure: f32,
    pub ambient: [f32; 3],
    pub time: f32,
    pub shadow_atlas: u32,
    pub hzb_image: u32,
    pub cluster_stride: u32,
    pub shadow_classes: u32,
    pub shadow_tint: u32,
    pub shadow_tint_depth: u32,
    pub shadow_atlas_size: f32,
    pub jitter: [f32; 2],
    pub mip_bias: f32,
    pub render_scale: f32,
    pub previous_jitter: [f32; 2],
    pub cut: u32,
    pub shadow_noise: u32,
    pub figure_atlas: u32,
    pub capacity: [f32; 2],
    pub previous_viewport: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuInstance {
    pub positions: u64,
    pub attributes: u64,
    pub transform: [f32; 16],
    pub normal_matrix: [[f32; 3]; 3],
    pub center: [f32; 3],
    pub radius: f32,
    pub material: u32,
    pub vertex_offset: u32,
    pub first_index: u32,
    pub index_count: u32,
    pub object_id: u32,
    pub flags: u32,
    pub bucket: u32,
    pub sort_key: u32,
    pub figure: u32,
    pub shadow_class: u32,
    pub layers: u32,
    pub wet: u64,
    pub outline_normals: u64,
    pub light_channels: u32,
    pub outline: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuMaterial {
    pub block: u32,
    pub alpha_cutoff: f32,
    pub albedo: u32,
    pub albedo_texture: u32,
    pub emission: u32,
    pub emission_texture: u32,
    pub roles: u32,
    pub albedo_weight: u32,
    pub metalness: u32,
    pub metalness_texture: u32,
    pub transmission: u32,
    pub emission_luminance: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuLight {
    pub position: [f32; 3],
    pub range: f32,
    pub direction: [f32; 3],
    pub intensity: f32,
    pub color: [f32; 3],
    pub kind: u32,
    pub tangent: [f32; 3],
    pub cone_scale: f32,
    pub extent: [f32; 2],
    pub cone_offset: f32,
    pub shadow: u32,
    pub shadow_count: u32,
    pub characters: u32,
    pub character_count: u32,
    pub channels: u32,
    pub figure: u32,
    pub flags: u32,
    pub size: f32,
    pub profile: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuShadowSlot {
    pub view_projection: [f32; 16],
    pub uv_scale: [f32; 2],
    pub uv_offset: [f32; 2],
    pub depth_bias: f32,
    pub normal_bias: f32,
    pub split_depth: f32,
    pub texel_size: f32,
    pub figure: u32,
    pub channels: u32,
    pub flags: u32,
    pub near: f32,
    pub projection_scale: f32,
    pub depth_range: f32,
    pub world_texel: f32,
    pub _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuCluster {
    pub count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuBucket {
    pub first: u32,
    pub capacity: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuDrawCommand {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub vertex_offset: i32,
    pub first_instance: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuFrameRefs {
    pub view: u64,
    pub instances: u64,
    pub materials: u64,
    pub lights: u64,
    pub shadows: u64,
    pub bones: u64,
    pub clusters: u64,
    pub cluster_indices: u64,
    pub buckets: u64,
    pub draw_commands: u64,
    pub draw_counts: u64,
    pub late_instances: u64,
    pub late_counter: u64,
    pub look: u64,
    pub figures: u64,
    pub environment: u64,
    pub previous_positions: u64,
    pub fluid: u64,
    pub blocks: u64,
    pub previous_transforms: u64,
    pub ltc: u64,
    pub profiles: u64,
    pub editor: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuPassPush {
    pub frame: u64,
    pub slot: u32,
    pub flags: u32,
}

const _: () = {
    use crate::frame_data::CLUSTER_MAX_LIGHTS;
    use rikka_gpu::core::placed_slots::PLACED_SLOTS;
    use rikka_gpu::core::sampler::{Filtering, SLOT_DEPTH_REDUCE, SLOT_SHADOW};
    use std::mem::size_of;

    use crate::vertex::{GpuAttribute, GpuMorphDelta, GpuPosition, GpuSkin};

    assert!(CLUSTER_TILE == 16);
    assert!(CLUSTER_SLICES == 24);
    assert!(NONE_INDEX == 0xffff_ffff);
    assert!(CLUSTER_MAX_LIGHTS == 128);
    assert!(Filtering::COUNT == 5);
    assert!(SLOT_SHADOW == 5);
    assert!(SLOT_DEPTH_REDUCE == 6);
    assert!(PLACED_SLOTS == 192);
    assert!(size_of::<GpuView>() == 608);
    assert!(size_of::<GpuPosition>() == 12);
    assert!(size_of::<GpuAttribute>() == 16);
    assert!(size_of::<GpuSkin>() == 16);
    assert!(size_of::<GpuMorphDelta>() == 32);
    assert!(size_of::<GpuInstance>() == 200);
    assert!(size_of::<GpuMaterial>() == 48);
    assert!(size_of::<GpuLight>() == 112);
    assert!(size_of::<GpuShadowSlot>() == 128);
    assert!(size_of::<GpuCluster>() == 4);
    assert!(size_of::<GpuBucket>() == 8);
    assert!(size_of::<GpuDrawCommand>() == 20);
    assert!(size_of::<GpuFrameRefs>() == 184);
    assert!(size_of::<GpuPassPush>() == 16);
};
