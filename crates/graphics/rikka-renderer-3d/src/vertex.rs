use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3, Vec4};
use half::f16;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuPosition {
    pub position: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuAttribute {
    pub normal: u32,
    pub tangent: u32,
    pub uv: [u16; 2],
    pub color: [u8; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuSkin {
    pub joints: [u16; 4],
    pub weights: [u16; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuMorphDelta {
    pub target: u32,
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub _pad: u32,
}

pub(crate) fn encode_octahedral(normal: Vec3) -> u32 {
    let n = normal.normalize_or(Vec3::Z);
    let sum = n.x.abs() + n.y.abs() + n.z.abs();
    let p = Vec2::new(n.x, n.y) / sum.max(f32::MIN_POSITIVE);
    let folded = match n.z >= 0.0 {
        true => p,
        false => Vec2::new(
            (1.0 - p.y.abs()) * signum_nonzero(p.x),
            (1.0 - p.x.abs()) * signum_nonzero(p.y),
        ),
    };
    let x = snorm16(folded.x) as u32;
    let y = snorm16(folded.y) as u32;
    x | (y << 16)
}

pub(crate) fn encode_tangent(tangent: Vec4) -> u32 {
    let packed = encode_octahedral(tangent.truncate()) & !1;
    packed | u32::from(tangent.w < 0.0)
}

fn signum_nonzero(value: f32) -> f32 {
    match value >= 0.0 {
        true => 1.0,
        false => -1.0,
    }
}

fn snorm16(value: f32) -> u16 {
    let clamped = value.clamp(-1.0, 1.0);
    ((clamped * 32767.0).round() as i16) as u16
}

impl GpuAttribute {
    pub(crate) fn new(normal: Vec3, tangent: Vec4, uv: Vec2, color: [u8; 4]) -> Self {
        Self {
            normal: encode_octahedral(normal),
            tangent: encode_tangent(tangent),
            uv: [f16::from_f32(uv.x).to_bits(), f16::from_f32(uv.y).to_bits()],
            color,
        }
    }
}

impl GpuSkin {
    pub(crate) fn new(joints: [u16; 4], weights: Vec4) -> Self {
        let sum = weights.x + weights.y + weights.z + weights.w;
        let normalized = match sum > 1e-6 {
            true => weights / sum,
            false => Vec4::X,
        };
        let quantized = [
            unorm16(normalized.x),
            unorm16(normalized.y),
            unorm16(normalized.z),
            unorm16(normalized.w),
        ];
        Self {
            joints,
            weights: fix_rounding(quantized),
        }
    }

    pub(crate) fn bound_joints(&self) -> impl Iterator<Item = u16> + '_ {
        self.joints
            .iter()
            .zip(self.weights)
            .filter(|&(_, weight)| weight > 0)
            .map(|(&joint, _)| joint)
    }
}
fn unorm16(value: f32) -> u16 {
    (value.clamp(0.0, 1.0) * 65535.0).round() as u16
}

fn fix_rounding(mut weights: [u16; 4]) -> [u16; 4] {
    let total: u32 = weights.iter().map(|&w| u32::from(w)).sum();
    let largest = weights
        .iter()
        .enumerate()
        .max_by_key(|&(_, &w)| w)
        .map_or(0, |(index, _)| index);
    let corrected = i64::from(weights[largest]) + 65535 - i64::from(total);
    weights[largest] = corrected.clamp(0, 65535) as u16;
    weights
}
