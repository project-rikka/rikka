use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Weak};

use ash::vk;
use rikka_sync::Mutex;

use crate::core::background::{Pool, Slot};
use crate::core::device::RenderDevice;
use crate::core::pipeline_cache::PersistentCache;
use crate::core::shader::create_shader_module;
use crate::error::{Error, Result};

pub const CANVAS_FORMATS: [vk::Format; 2] =
    [vk::Format::B8G8R8A8_UNORM, vk::Format::R8G8B8A8_UNORM];

const MAX_PIPELINE_WORKERS: usize = 4;

type Build<T> = Arc<dyn Fn(&Arc<RenderDevice>) -> Result<T> + Send + Sync>;

type Watcher = Box<dyn FnOnce() + Send>;

static PIPELINES: Pool = Pool::new("rikka-pipelines", pipeline_workers);

fn pipeline_workers() -> usize {
    std::thread::available_parallelism()
        .map_or(1, |cores| cores.get() / 2)
        .clamp(1, MAX_PIPELINE_WORKERS)
}

struct Shared<T> {
    slot: Slot<Result<T>>,
    watchers: Mutex<Vec<Watcher>>,
}

impl<T> Shared<T> {
    fn release_watchers(&self) {
        let watchers = std::mem::take(&mut *self.watchers.lock());
        watchers.into_iter().for_each(|watcher| watcher());
    }
}

pub struct Compiling<T> {
    device: Arc<RenderDevice>,
    build: Build<T>,
    shared: Arc<Shared<T>>,
    built: Option<T>,
    failure: Option<String>,
}

impl<T: Send + 'static> Compiling<T> {
    pub fn start(
        device: &Arc<RenderDevice>,
        build: impl Fn(&Arc<RenderDevice>) -> Result<T> + Send + Sync + 'static,
    ) -> Self {
        let compiling = Self {
            device: device.clone(),
            build: Arc::new(build),
            shared: Arc::new(Shared {
                slot: Slot::queued(),
                watchers: Mutex::new(Vec::new()),
            }),
            built: None,
            failure: None,
        };
        compiling.dispatch();
        compiling
    }

    pub fn ready(&self) -> bool {
        self.built.is_some() || self.failure.is_some() || self.shared.slot.is_done()
    }

    pub fn when_ready(&self, notify: impl FnOnce() + Send + 'static) {
        if self.built.is_some() || self.failure.is_some() {
            notify();
            return;
        }
        let notify: Watcher = Box::new(notify);
        let mut waiting = Some(notify);
        self.shared
            .slot
            .unless_done(|| self.shared.watchers.lock().extend(waiting.take()));
        if let Some(notify) = waiting {
            notify();
        }
    }

    pub fn get(&mut self) -> Result<&T> {
        if let Some(failure) = &self.failure {
            return Err(Error::Driver(failure.clone().into()));
        }
        if self.built.is_none() {
            let settled = match self.shared.slot.claim() {
                Some(built) => built,
                None => (self.build)(&self.device),
            };
            self.shared.release_watchers();
            match settled {
                Ok(built) => self.built = Some(built),
                Err(error) => {
                    self.failure = Some(describe(&error));
                    return Err(error);
                }
            }
        }
        Ok(self.built.as_ref().expect("compile result is ready"))
    }

    fn dispatch(&self) {
        let shared = Arc::clone(&self.shared);
        let build = self.build.clone();
        let device = Arc::downgrade(&self.device);
        PIPELINES.enqueue(move || {
            let ran = shared.slot.run(|| {
                let device = device
                    .upgrade()
                    .expect("queued build keeps its device alive");
                catch_unwind(AssertUnwindSafe(|| build(&device)))
                    .unwrap_or_else(|_| Err(Error::Invalid("pipeline build panicked".to_owned())))
            });
            if ran {
                shared.release_watchers();
            }
        });
    }
}

impl<T> std::fmt::Debug for Compiling<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Compiling")
            .field("built", &self.built.is_some())
            .finish_non_exhaustive()
    }
}

impl<T> Drop for Compiling<T> {
    fn drop(&mut self) {
        drop(self.shared.slot.claim());
    }
}

fn describe(error: &Error) -> String {
    let mut text = error.to_string();
    let mut cause = std::error::Error::source(error);
    while let Some(inner) = cause {
        text.push_str(": ");
        text.push_str(&inner.to_string());
        cause = inner.source();
    }
    text
}

pub(crate) fn save_in_background(cache: Weak<PersistentCache>) {
    PIPELINES.enqueue(move || {
        let Some(cache) = cache.upgrade() else {
            return;
        };
        if let Err(error) = cache.save() {
            tracing::warn!(%error, "failed to save pipeline cache");
        }
    });
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Blend {
    #[default]
    Opaque,
    Premultiplied,
    Alpha,
    Additive,
    Accumulate,
    Mix,
    Multiply,
}

pub struct GraphicsPipelineDesc<'a> {
    pub vertex_spv: &'a [u8],
    pub fragment_spv: &'a [u8],
    pub set_layouts: &'a [vk::DescriptorSetLayout],
    pub push_constant_size: u32,
    pub push_constant_stages: vk::ShaderStageFlags,
    pub color_format: vk::Format,
    pub blend: Blend,
}

#[derive(Clone, Copy, Debug)]
pub struct PipelineHandles {
    pub layout: vk::PipelineLayout,
    pub pipeline: vk::Pipeline,
}

pub struct Pipeline {
    device: Arc<RenderDevice>,
    handles: PipelineHandles,
}

impl Pipeline {
    pub fn build(
        device: &Arc<RenderDevice>,
        name: &str,
        desc: &GraphicsPipelineDesc<'_>,
    ) -> Result<Self> {
        let handles = create_graphics_pipeline(device, desc)?;
        device.set_object_name(handles.pipeline, name);
        Ok(Self {
            device: device.clone(),
            handles,
        })
    }

    pub fn handles(&self) -> PipelineHandles {
        self.handles
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        unsafe { destroy_pipeline(&self.device, self.handles) };
    }
}

fn create_graphics_pipeline(
    device: &RenderDevice,
    desc: &GraphicsPipelineDesc<'_>,
) -> Result<PipelineHandles> {
    let (vertex, fragment) =
        create_stage_modules(device, desc.vertex_spv, Some(desc.fragment_spv))?;
    let fragment = fragment.expect("fragment stage is present");

    let result = build(device, desc, vertex, fragment);

    unsafe { destroy_stage_modules(device, vertex, Some(fragment)) };

    result
}

fn build(
    device: &RenderDevice,
    desc: &GraphicsPipelineDesc<'_>,
    vertex: vk::ShaderModule,
    fragment: vk::ShaderModule,
) -> Result<PipelineHandles> {
    let mut push_ranges = Vec::new();
    if desc.push_constant_size > 0 {
        push_ranges.push(
            vk::PushConstantRange::default()
                .stage_flags(desc.push_constant_stages)
                .offset(0)
                .size(desc.push_constant_size),
        );
    }
    let layout_info = vk::PipelineLayoutCreateInfo::default()
        .set_layouts(desc.set_layouts)
        .push_constant_ranges(&push_ranges);
    let layout = unsafe { device.handle().create_pipeline_layout(&layout_info, None)? };

    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vertex)
            .name(c"main"),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fragment)
            .name(c"main"),
    ];

    let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
    let viewport = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);
    let rasterization = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(vk::CullModeFlags::NONE)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .line_width(1.0);
    let multisample = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);

    let blend_attachments = [blend_state(desc.blend)];
    let color_blend =
        vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_attachments);

    let dynamic_states = [
        vk::DynamicState::VIEWPORT,
        vk::DynamicState::SCISSOR,
        vk::DynamicState::BLEND_CONSTANTS,
    ];
    let dynamic_count = match desc.blend {
        Blend::Mix => dynamic_states.len(),
        _ => dynamic_states.len() - 1,
    };
    let dynamic = vk::PipelineDynamicStateCreateInfo::default()
        .dynamic_states(&dynamic_states[..dynamic_count]);

    let color_formats = [desc.color_format];
    let mut rendering =
        vk::PipelineRenderingCreateInfo::default().color_attachment_formats(&color_formats);

    let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vertex_input)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport)
        .rasterization_state(&rasterization)
        .multisample_state(&multisample)
        .color_blend_state(&color_blend)
        .dynamic_state(&dynamic)
        .layout(layout)
        .push_next(&mut rendering);

    let pipelines = unsafe {
        device
            .handle()
            .create_graphics_pipelines(device.pipeline_cache(), &[pipeline_info], None)
    };
    match pipelines {
        Ok(pipelines) => Ok(PipelineHandles {
            layout,
            pipeline: pipelines[0],
        }),
        Err((_, err)) => {
            unsafe { device.handle().destroy_pipeline_layout(layout, None) };
            Err(err.into())
        }
    }
}

pub fn blend_state(blend: Blend) -> vk::PipelineColorBlendAttachmentState {
    let base = vk::PipelineColorBlendAttachmentState::default()
        .color_write_mask(vk::ColorComponentFlags::RGBA);
    match blend {
        Blend::Opaque => base.blend_enable(false),
        Blend::Premultiplied => base
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD),
        Blend::Alpha => base
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD),
        Blend::Additive => base
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ONE)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ZERO)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE)
            .alpha_blend_op(vk::BlendOp::ADD),
        Blend::Accumulate => base
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ONE)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE)
            .alpha_blend_op(vk::BlendOp::ADD),
        Blend::Mix => base
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::ONE_MINUS_CONSTANT_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::CONSTANT_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_CONSTANT_ALPHA)
            .dst_alpha_blend_factor(vk::BlendFactor::CONSTANT_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD),
        Blend::Multiply => base
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::ZERO)
            .dst_color_blend_factor(vk::BlendFactor::SRC_COLOR)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ZERO)
            .dst_alpha_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD),
    }
}

pub fn create_stage_modules(
    device: &RenderDevice,
    vertex_spv: &[u8],
    fragment_spv: Option<&[u8]>,
) -> Result<(vk::ShaderModule, Option<vk::ShaderModule>)> {
    let vertex = create_shader_module(device, vertex_spv)?;
    let fragment = match fragment_spv {
        Some(spv) => match create_shader_module(device, spv) {
            Ok(module) => Some(module),
            Err(err) => {
                unsafe { device.handle().destroy_shader_module(vertex, None) };
                return Err(err);
            }
        },
        None => None,
    };
    Ok((vertex, fragment))
}

pub unsafe fn destroy_stage_modules(
    device: &RenderDevice,
    vertex: vk::ShaderModule,
    fragment: Option<vk::ShaderModule>,
) {
    unsafe {
        device.handle().destroy_shader_module(vertex, None);
        if let Some(fragment) = fragment {
            device.handle().destroy_shader_module(fragment, None);
        }
    }
}

unsafe fn destroy_pipeline(device: &RenderDevice, handles: PipelineHandles) {
    unsafe {
        device.handle().destroy_pipeline(handles.pipeline, None);
        device
            .handle()
            .destroy_pipeline_layout(handles.layout, None);
    }
}

impl<'a> std::fmt::Debug for GraphicsPipelineDesc<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphicsPipelineDesc")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Pipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pipeline").finish_non_exhaustive()
    }
}
