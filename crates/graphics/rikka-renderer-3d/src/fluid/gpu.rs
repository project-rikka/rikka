use bytemuck::{Pod, Zeroable};

pub const PARTICLE_BYTES: u64 = 64;
pub const SPRAY_BYTES: u64 = 32;
pub const WET_BYTES: u64 = 16;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuFluidLayer {
    pub particles: u64,
    pub spray: u64,
    pub count: u32,
    pub spray_count: u32,
    pub material_first: u32,
    pub material_count: u32,
    pub radius: f32,
    pub volume: f32,
    pub _pad: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuFluidMaterial {
    pub absorption: [f32; 3],
    pub scattering: f32,
    pub albedo: [f32; 3],
    pub roughness: f32,
    pub ior: f32,
    pub extinction: f32,
    pub _pad: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuFluidView {
    pub view_projection: [f32; 16],
    pub inverse: [f32; 16],
    pub texel: [f32; 2],
    pub perspective: u32,
    pub _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuFluid {
    pub layers: u64,
    pub materials: u64,
    pub views: u64,
    pub previous_view_projection: [f32; 16],
    pub depth: u32,
    pub front: u32,
    pub smoothed: u32,
    pub thickness: u32,
    pub scattering: u32,
    pub scene_depth: u32,
    pub layer_count: u32,
    pub view_count: u32,
    pub radius: f32,
    pub delta_time: f32,
    pub smoothing: f32,
    pub coverage: u32,
}

const _: () = {
    assert!(size_of::<GpuFluidLayer>() == 48);
    assert!(size_of::<GpuFluidMaterial>() == 48);
    assert!(size_of::<GpuFluidView>() == 144);
    assert!(size_of::<GpuFluid>() == 136);
};
