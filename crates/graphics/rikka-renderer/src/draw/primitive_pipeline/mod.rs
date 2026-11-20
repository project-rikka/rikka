mod colors;
mod store;
mod upload;

use std::sync::Arc;

use ash::vk;
use rustc_hash::FxHashMap;

use crate::scene::SceneId;
use rikka_gpu::core::buffer::GpuBuffer;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame::MAX_FRAMES_IN_FLIGHT;
use rikka_gpu::core::memory::Placement;
use rikka_gpu::core::pipeline::{
    Blend, CANVAS_FORMATS, Compiling, GraphicsPipelineDesc, Pipeline, PipelineHandles,
};
use rikka_gpu::core::runtime::Recycler;
use rikka_gpu::error::Result;

use colors::ColorTable;
use upload::{InstanceUpload, SetBindings};

pub(crate) use store::Store;

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct PrimitivePush {
    target: [f32; 4],
    glass: GlassPush,
}

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct GlassPush {
    uv: [f32; 4],
    bounds: [f32; 4],
    adapt: f32,
    tex: u32,
}

const GLASS_OFFSET: u32 = size_of::<[f32; 4]>() as u32;

const NO_GLASS: u32 = u32::MAX;

const _: () = assert!(size_of::<PrimitivePush>() == 56);

const QUAD: [u16; 6] = [0, 1, 2, 2, 1, 3];

#[derive(Clone, Copy, Debug)]
pub(crate) struct Glass {
    pub tex: u32,
    pub uv: [f32; 4],
    pub bounds: [f32; 4],
    pub adapt: f32,
}

pub(crate) struct PrimitivePipeline {
    device: Arc<RenderDevice>,
    instance_set_layout: vk::DescriptorSetLayout,
    table_layout: vk::DescriptorSetLayout,
    pipelines: FxHashMap<vk::Format, Compiling<Pipeline>>,
    descriptor_pool: vk::DescriptorPool,
    instance_sets: [vk::DescriptorSet; MAX_FRAMES_IN_FLIGHT],
    stores: FxHashMap<Option<SceneId>, Store>,
    unconfirmed: Option<Option<SceneId>>,
    quad: GpuBuffer,
    sending: InstanceUpload,
    colors: ColorTable,
    written: [Option<SetBindings>; MAX_FRAMES_IN_FLIGHT],
    bound: Option<vk::PipelineLayout>,
}

impl PrimitivePipeline {
    pub(crate) fn new(
        device: Arc<RenderDevice>,
        table_layout: vk::DescriptorSetLayout,
    ) -> Result<Self> {
        let mut quad = GpuBuffer::new(
            device.clone(),
            "rikka.quad",
            size_of_val(&QUAD) as vk::DeviceSize,
            vk::BufferUsageFlags::INDEX_BUFFER,
            Placement::Upload,
        )?;
        quad.mapped_slice_mut()
            .expect("index memory is host-visible")[..size_of_val(&QUAD)]
            .copy_from_slice(bytemuck::cast_slice(&QUAD));

        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX),
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX),
            vk::DescriptorSetLayoutBinding::default()
                .binding(3)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX),
            vk::DescriptorSetLayoutBinding::default()
                .binding(4)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(5)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX),
            vk::DescriptorSetLayoutBinding::default()
                .binding(6)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(7)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
        ];
        let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let instance_set_layout = unsafe {
            device
                .handle()
                .create_descriptor_set_layout(&layout_info, None)?
        };

        let pool_sizes = [vk::DescriptorPoolSize {
            ty: vk::DescriptorType::STORAGE_BUFFER,
            descriptor_count: (MAX_FRAMES_IN_FLIGHT * bindings.len()) as u32,
        }];
        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(MAX_FRAMES_IN_FLIGHT as u32)
            .pool_sizes(&pool_sizes);
        let descriptor_pool = unsafe { device.handle().create_descriptor_pool(&pool_info, None) };
        let descriptor_pool = match descriptor_pool {
            Ok(pool) => pool,
            Err(err) => {
                unsafe {
                    device
                        .handle()
                        .destroy_descriptor_set_layout(instance_set_layout, None)
                };
                return Err(err.into());
            }
        };

        let layouts = [instance_set_layout; MAX_FRAMES_IN_FLIGHT];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&layouts);
        let sets = unsafe { device.handle().allocate_descriptor_sets(&alloc_info) };
        let sets = match sets {
            Ok(sets) => sets,
            Err(err) => {
                unsafe {
                    device
                        .handle()
                        .destroy_descriptor_pool(descriptor_pool, None);
                    device
                        .handle()
                        .destroy_descriptor_set_layout(instance_set_layout, None);
                }
                return Err(err.into());
            }
        };

        let layouts = [instance_set_layout, table_layout];
        let pipelines = CANVAS_FORMATS
            .into_iter()
            .map(|format| (format, compile(&device, layouts, format)))
            .collect();
        Ok(Self {
            device,
            instance_set_layout,
            table_layout,
            pipelines,
            descriptor_pool,
            instance_sets: sets
                .try_into()
                .expect("one descriptor set per frame in flight"),
            stores: FxHashMap::default(),
            unconfirmed: None,
            quad,
            sending: InstanceUpload::default(),
            colors: ColorTable::default(),
            written: [None; MAX_FRAMES_IN_FLIGHT],
            bound: None,
        })
    }

    pub(crate) fn trim(&mut self, recycler: &mut Recycler<'_>) {
        self.sending.shuffle.trim(recycler);
    }

    pub(crate) fn forget(&mut self, scene: SceneId) -> Option<Store> {
        self.stores.remove(&Some(scene))
    }

    pub(crate) fn bind(
        &mut self,
        cmd: vk::CommandBuffer,
        format: vk::Format,
        instance_set: vk::DescriptorSet,
        table_set: vk::DescriptorSet,
        target_size: [f32; 2],
        placement: [f32; 2],
    ) -> Result<()> {
        let handles = self.pipeline_for(format)?;
        self.bound = Some(handles.layout);
        let push = PrimitivePush {
            target: [target_size[0], target_size[1], placement[0], placement[1]],
            glass: GlassPush {
                tex: NO_GLASS,
                ..GlassPush::default()
            },
        };
        let device = self.device.handle();
        unsafe {
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, handles.pipeline);
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                handles.layout,
                0,
                &[instance_set, table_set],
                &[],
            );
            device.cmd_bind_index_buffer(cmd, self.quad.handle(), 0, vk::IndexType::UINT16);
            device.cmd_push_constants(
                cmd,
                handles.layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::bytes_of(&push),
            );
        }
        Ok(())
    }

    pub(crate) fn set_glass(&self, cmd: vk::CommandBuffer, glass: Option<Glass>) {
        let layout = self.bound.expect("pipeline bound before draw");
        let push = match glass {
            Some(glass) => GlassPush {
                uv: glass.uv,
                bounds: glass.bounds,
                adapt: glass.adapt,
                tex: glass.tex,
            },
            None => GlassPush {
                tex: NO_GLASS,
                ..GlassPush::default()
            },
        };
        unsafe {
            self.device.handle().cmd_push_constants(
                cmd,
                layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                GLASS_OFFSET,
                bytemuck::bytes_of(&push),
            );
        }
    }

    pub(crate) fn draw(&self, cmd: vk::CommandBuffer, first: u32, count: u32) {
        if count == 0 {
            return;
        }
        unsafe {
            self.device
                .handle()
                .cmd_draw_indexed(cmd, QUAD.len() as u32, count, 0, 0, first)
        };
    }

    fn pipeline_for(&mut self, format: vk::Format) -> Result<PipelineHandles> {
        let layouts = [self.instance_set_layout, self.table_layout];
        let device = &self.device;
        let compiling = self
            .pipelines
            .entry(format)
            .or_insert_with(|| compile(device, layouts, format));
        Ok(compiling.get()?.handles())
    }
}

fn compile(
    device: &Arc<RenderDevice>,
    set_layouts: [vk::DescriptorSetLayout; 2],
    format: vk::Format,
) -> Compiling<Pipeline> {
    Compiling::start(device, move |device| {
        Pipeline::build(
            device,
            "rikka.primitive",
            &GraphicsPipelineDesc {
                vertex_spv: shaders::PRIMITIVE_VERT.spirv,
                fragment_spv: shaders::PRIMITIVE_FRAG.spirv,
                set_layouts: &set_layouts,
                push_constant_size: size_of::<PrimitivePush>() as u32,
                push_constant_stages: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                color_format: format,
                blend: Blend::Premultiplied,
            },
        )
    })
}

impl Drop for PrimitivePipeline {
    fn drop(&mut self) {
        self.pipelines.clear();
        unsafe {
            self.device
                .handle()
                .destroy_descriptor_pool(self.descriptor_pool, None);
            self.device
                .handle()
                .destroy_descriptor_set_layout(self.instance_set_layout, None);
        }
    }
}
