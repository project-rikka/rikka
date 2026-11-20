use std::sync::Arc;

use ash::vk;

use crate::core::device::RenderDevice;
use crate::core::frame_ctx::GpuFrameCtx;
use crate::core::image::{GpuImage, ImageAccess, record_barriers};
use crate::core::pipeline::{Blend, Compiling, GraphicsPipelineDesc, Pipeline};
use crate::core::sampler::Filtering;
use crate::error::Result;

pub const FX_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
pub(crate) const DEEP_FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ConvertPush {
    pub rows: [[f32; 4]; 3],
    pub uv_scale: [f32; 2],
    pub chroma_packed: bool,
    pub chroma_swapped: bool,
    pub chroma_offset: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ConvertPlane {
    pub view: vk::ImageView,
    pub linear: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ShaderPush {
    rows: [[f32; 4]; 3],
    uv_scale: [f32; 2],
    chroma_packed: u32,
    chroma_swapped: u32,
    planes: [u32; 4],
    chroma_offset: [f32; 2],
}

pub fn ycbcr_matrix(
    luma: f64,
    blue: f64,
    bit_depth: u32,
    storage_scale: f64,
    full_range: bool,
) -> [[f32; 4]; 3] {
    let (kr, kb) = (luma, blue);
    let kg = 1.0 - kr - kb;

    let max = f64::from((1u32 << bit_depth) - 1);
    let (luma_offset, luma_scale, chroma_scale) = if full_range {
        (0.0, 1.0, 1.0)
    } else {
        (
            f64::from(16u32 << (bit_depth - 8)) / max,
            f64::from(219u32 << (bit_depth - 8)) / max,
            f64::from(224u32 << (bit_depth - 8)) / max,
        )
    };
    let chroma_offset = f64::from(1u32 << (bit_depth - 1)) / max;

    let m = [
        [1.0, 0.0, 2.0 * (1.0 - kr)],
        [
            1.0,
            -2.0 * kb * (1.0 - kb) / kg,
            -2.0 * kr * (1.0 - kr) / kg,
        ],
        [1.0, 2.0 * (1.0 - kb), 0.0],
    ];
    let scales = [
        storage_scale / luma_scale,
        storage_scale / chroma_scale,
        storage_scale / chroma_scale,
    ];
    let offsets = [
        luma_offset / luma_scale,
        chroma_offset / chroma_scale,
        chroma_offset / chroma_scale,
    ];

    let mut rows = [[0.0f32; 4]; 3];
    for (row, coefficients) in rows.iter_mut().zip(m.iter()) {
        let mut bias = 0.0;
        for column in 0..3 {
            row[column] = (coefficients[column] * scales[column]) as f32;
            bias -= coefficients[column] * offsets[column];
        }
        row[3] = bias as f32;
    }
    rows
}

pub struct YcbcrConverter {
    device: Arc<RenderDevice>,
    table_layout: vk::DescriptorSetLayout,
    pipeline: Compiling<Pipeline>,
    deep: Option<Compiling<Pipeline>>,
}

impl YcbcrConverter {
    pub fn new(device: Arc<RenderDevice>, table_layout: vk::DescriptorSetLayout) -> Self {
        Self {
            pipeline: Compiling::start(&device, move |device| {
                build_pipeline(device, table_layout, FX_FORMAT)
            }),
            deep: None,
            device,
            table_layout,
        }
    }

    pub(crate) fn ready(&self) -> bool {
        self.pipeline.ready()
    }

    pub(crate) fn record(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        sources: &[vk::ImageMemoryBarrier2<'static>],
        target: &mut GpuImage,
        planes: [ConvertPlane; 3],
        push: ConvertPush,
    ) -> Result<()> {
        let handles = match target.format() {
            DEEP_FORMAT => {
                let table_layout = self.table_layout;
                self.deep
                    .get_or_insert_with(|| {
                        Compiling::start(&self.device, move |device| {
                            build_pipeline(device, table_layout, DEEP_FORMAT)
                        })
                    })
                    .get()?
                    .handles()
            }
            _ => self.pipeline.get()?.handles(),
        };
        let push = ShaderPush {
            rows: push.rows,
            uv_scale: push.uv_scale,
            chroma_packed: push.chroma_packed.into(),
            chroma_swapped: push.chroma_swapped.into(),
            chroma_offset: push.chroma_offset,
            planes: bind_planes(ctx, planes)?,
        };

        ctx.begin_pass("rikka.convert");
        let cmd = ctx.cmd;
        let device = ctx.device.handle();
        target.discard();
        let mut barriers = sources.to_vec();
        barriers.extend(target.transition(ImageAccess::COLOR_ATTACHMENT));
        record_barriers(ctx.device, cmd, &barriers);

        let extent = target.extent();
        let attachments = [vk::RenderingAttachmentInfo::default()
            .image_view(target.view())
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::DONT_CARE)
            .store_op(vk::AttachmentStoreOp::STORE)];
        let area = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent,
        };
        let rendering = vk::RenderingInfo::default()
            .render_area(area)
            .layer_count(1)
            .color_attachments(&attachments);

        unsafe {
            device.cmd_begin_rendering(cmd, &rendering);
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, handles.pipeline);
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                handles.layout,
                0,
                &[ctx.resources.set()],
                &[],
            );
            device.cmd_push_constants(
                cmd,
                handles.layout,
                vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::bytes_of(&push),
            );
            device.cmd_set_viewport(
                cmd,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: extent.width as f32,
                    height: extent.height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            device.cmd_set_scissor(cmd, 0, &[area]);
            device.cmd_draw(cmd, 3, 1, 0, 0);
            device.cmd_end_rendering(cmd);
        }

        target.barrier_to(cmd, ImageAccess::TABLE_SAMPLED);
        ctx.end_pass();
        Ok(())
    }
}

impl std::fmt::Debug for YcbcrConverter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("YcbcrConverter")
            .field("pipeline", &self.pipeline)
            .finish()
    }
}

fn bind_planes(ctx: &mut GpuFrameCtx<'_>, planes: [ConvertPlane; 3]) -> Result<[u32; 4]> {
    let mut slots = [0u32; 4];
    let mut bound: Vec<(ConvertPlane, u32)> = Vec::with_capacity(planes.len());
    for (index, plane) in planes.into_iter().enumerate() {
        let slot = match bound.iter().find(|(bound_plane, _)| *bound_plane == plane) {
            Some(&(_, slot)) => slot,
            None => {
                let filtering = match plane.linear {
                    true => Filtering::LinearClamp,
                    false => Filtering::NearestClamp,
                };
                let sampler = ctx.samplers.table()[filtering.slot() as usize];
                let slot = ctx.resources.allocate(plane.view, sampler)?;
                ctx.resources.release(slot, ctx.frame_value);
                bound.push((plane, slot));
                slot
            }
        };
        slots[index] = slot;
    }
    Ok(slots)
}

fn build_pipeline(
    device: &Arc<RenderDevice>,
    table_layout: vk::DescriptorSetLayout,
    color_format: vk::Format,
) -> Result<Pipeline> {
    let set_layouts = [table_layout];
    Pipeline::build(
        device,
        "rikka.convert.ycbcr",
        &GraphicsPipelineDesc {
            vertex_spv: shaders_gpu::FULLSCREEN_VERT.spirv,
            fragment_spv: shaders_gpu::YCBCR_CONVERT_FRAG.spirv,
            set_layouts: &set_layouts,
            push_constant_size: std::mem::size_of::<ShaderPush>() as u32,
            push_constant_stages: vk::ShaderStageFlags::FRAGMENT,
            color_format,
            blend: Blend::Opaque,
        },
    )
}
