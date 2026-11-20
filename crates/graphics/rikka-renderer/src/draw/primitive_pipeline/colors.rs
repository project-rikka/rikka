use std::sync::Arc;

use ash::vk;
use rikka_colorimetry::{BlendSpace, Encoding, Mapping, PQ_PEAK, TransferFunction};

use crate::renderer::{AlphaMode, Colors};
use rikka_gpu::core::buffer::{BufferBinding, GpuBuffer};
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame::MAX_FRAMES_IN_FLIGHT;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::memory::Placement;
use rikka_gpu::error::Result;

const DECODE_IDENTITY: u32 = 0;
const DECODE_POWER: u32 = 1;
const DECODE_COMPOUND_POWER: u32 = 2;
const DECODE_PQ: u32 = 3;
const DECODE_HLG: u32 = 4;
const DECODE_LINEAR: u32 = 5;

const ALPHA_PREMULTIPLIED: u32 = 0;
const ALPHA_OPTICAL: u32 = 1;
const ALPHA_STRAIGHT: u32 = 2;

const ENCODE_GAMMA22: u32 = 0;
const ENCODE_LINEAR: u32 = 1;

const MIN_ENTRIES: usize = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct GpuConversion {
    mode: [u32; 4],
    decode: [f32; 4],
    rows: [[f32; 4]; 3],
    tone: [f32; 4],
}

const _: () = assert!(size_of::<GpuConversion>() == 96);

impl GpuConversion {
    const IDENTITY: Self = Self {
        mode: [DECODE_IDENTITY, ALPHA_PREMULTIPLIED, 0, ENCODE_GAMMA22],
        decode: [0.0; 4],
        rows: [[0.0; 4]; 3],
        tone: [0.0; 4],
    };

    fn new(mapping: &Mapping, alpha: AlphaMode) -> Self {
        if mapping.is_identity() && alpha == AlphaMode::Premultiplied {
            return Self::IDENTITY;
        }
        let (kind, exponent) = match mapping.decode {
            TransferFunction::Power(exponent) => (DECODE_POWER, exponent),
            TransferFunction::Linear => (DECODE_LINEAR, 1.0),
            TransferFunction::CompoundPower24 => (DECODE_COMPOUND_POWER, 1.0),
            TransferFunction::Pq => (DECODE_PQ, 1.0),
            TransferFunction::Hlg => (DECODE_HLG, 1.0),
        };
        let (ootf_exponent, ootf_luminance, ootf) = match mapping.ootf {
            Some(ootf) => (ootf.gamma - 1.0, ootf.luminance, 1.0),
            None => (0.0, [0.0; 3], 0.0),
        };
        let rows = mapping.matrix.rows();
        let scale = mapping.scale;
        let tone = mapping.tone.map_or([0.0; 4], |tone| {
            [
                (tone.anchor / PQ_PEAK) as f32,
                tone.source_peak as f32,
                tone.target_peak as f32,
                tone.knee as f32,
            ]
        });
        Self {
            mode: [
                kind,
                match alpha {
                    AlphaMode::Premultiplied => ALPHA_PREMULTIPLIED,
                    AlphaMode::PremultipliedOptical => ALPHA_OPTICAL,
                    AlphaMode::Straight => ALPHA_STRAIGHT,
                },
                u32::from(mapping.tone.is_some()),
                match mapping.encoding {
                    Encoding::Gamma22 => ENCODE_GAMMA22,
                    Encoding::Linear => ENCODE_LINEAR,
                },
            ],
            decode: [exponent as f32, 0.0, ootf_exponent as f32, ootf],
            rows: std::array::from_fn(|row| {
                [
                    (rows[row][0] * scale) as f32,
                    (rows[row][1] * scale) as f32,
                    (rows[row][2] * scale) as f32,
                    ootf_luminance[row] as f32,
                ]
            }),
            tone,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Written {
    revision: u64,
    space: BlendSpace,
}

#[derive(Default)]
pub(super) struct ColorTable {
    buffers: [Option<GpuBuffer>; MAX_FRAMES_IN_FLIGHT],
    written: [Option<Written>; MAX_FRAMES_IN_FLIGHT],
    scratch: Vec<GpuConversion>,
}

impl ColorTable {
    pub(super) fn bind(
        &mut self,
        device: &Arc<RenderDevice>,
        ctx: &mut GpuFrameCtx<'_>,
        colors: &Colors,
        space: &BlendSpace,
    ) -> Result<BufferBinding> {
        let slot = ctx.frame_slot;
        let entries = colors.len().max(MIN_ENTRIES).next_power_of_two();
        let required = (entries * size_of::<GpuConversion>()) as vk::DeviceSize;
        if self.buffers[slot]
            .as_ref()
            .is_none_or(|buffer| buffer.size() < required)
        {
            let buffer = GpuBuffer::new(
                device.clone(),
                "rikka.conversions",
                required,
                vk::BufferUsageFlags::STORAGE_BUFFER,
                Placement::Upload,
            )?;
            if let Some(old) = self.buffers[slot].replace(buffer) {
                ctx.retire.defer(ctx.frame_value, Box::new(old));
            }
            self.written[slot] = None;
        }
        let wanted = Written {
            revision: colors.revision(),
            space: *space,
        };
        let buffer = self.buffers[slot]
            .as_mut()
            .expect("conversion buffer exists for the frame slot");
        if self.written[slot] != Some(wanted) {
            self.scratch.clear();
            self.scratch
                .extend(colors.mappings(space).map(|entry| match entry {
                    Some((mapping, alpha)) => GpuConversion::new(&mapping, alpha),
                    None => GpuConversion::IDENTITY,
                }));
            let bytes: &[u8] = bytemuck::cast_slice(&self.scratch);
            buffer
                .mapped_slice_mut()
                .expect("conversion memory is host-visible")[..bytes.len()]
                .copy_from_slice(bytes);
            self.written[slot] = Some(wanted);
        }
        Ok(buffer.binding())
    }
}
