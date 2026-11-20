use bytemuck::{Pod, Zeroable};

use crate::look::{CURVE_KEYS, MAX_BURSTS};

pub(crate) const PARTICLE_BYTES: u64 = 64;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuBurst {
    pub time: f32,
    pub duration: f32,
    pub count: u32,
    pub pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GpuEmitterKey {
    pub translation: [f32; 3],
    pub time: f32,
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GpuEmitter {
    pub first_particle: u32,
    pub slots: u32,
    pub capacity: u32,
    pub seed: u32,
    pub first_key: u32,
    pub key_count: u32,
    pub burst_count: u32,
    pub shape: u32,
    pub start: f32,
    pub end: f32,
    pub rate: f32,
    pub lifetime: f32,
    pub lifetime_variance: f32,
    pub speed: f32,
    pub speed_variance: f32,
    pub inherit: f32,
    pub launch: [f32; 3],
    pub radius: f32,
    pub extent: [f32; 3],
    pub angle: f32,
    pub gravity: [f32; 3],
    pub drag: f32,
    pub wind: [f32; 3],
    pub drag_variance: f32,
    pub noise_offset: [f32; 3],
    pub turbulence: f32,
    pub attractor: [f32; 3],
    pub attraction: f32,
    pub frequency: f32,
    pub flow: f32,
    pub attraction_rate: f32,
    pub ground: u32,
    pub ground_height: f32,
    pub restitution: f32,
    pub friction: f32,
    pub ground_fade: f32,
    pub size_variance: f32,
    pub rotation: f32,
    pub spin: f32,
    pub spin_variance: f32,
    pub stretch: f32,
    pub stretch_min: f32,
    pub stretch_max: f32,
    pub softness: f32,
    pub twinkle: f32,
    pub twinkle_rate: f32,
    pub translucency: f32,
    pub layers: u32,
    pub axis: [f32; 3],
    pub facing: u32,
    pub sprite: u32,
    pub sampler: u32,
    pub columns: u32,
    pub rows: u32,
    pub frame_rate: f32,
    pub random_frame: u32,
    pub blend: u32,
    pub lit: u32,
    pub bloom: u32,
    pub look: u32,
    pub coloring: u32,
    pub aspect: f32,
    pub size_ages: [f32; CURVE_KEYS],
    pub size_values: [f32; CURVE_KEYS],
    pub opacity_ages: [f32; CURVE_KEYS],
    pub opacity_values: [f32; CURVE_KEYS],
    pub emission_ages: [f32; CURVE_KEYS],
    pub emission_values: [f32; CURVE_KEYS],
    pub color_ages: [f32; CURVE_KEYS],
    pub colors: [[f32; 4]; CURVE_KEYS],
    pub bursts: [GpuBurst; MAX_BURSTS],
}

const _: () = assert!(size_of::<GpuEmitter>() == 736);
const _: () = assert!(size_of::<GpuEmitterKey>() == 48);
