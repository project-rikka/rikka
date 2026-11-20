use std::sync::Arc;

use ash::vk;
use rustc_hash::FxHashMap;

use crate::material::features::ALL_FEATURES;
use crate::material::{Cull, MaterialState, Polygon};
use crate::record::SURFACE_FORMAT;
use crate::shader::MaterialShader;
use crate::viewport::{IDENTITY_FORMAT, OUTLINE_FORMAT};
use rikka_gpu::core::compute::create_compute_pipeline;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::pipeline::{
    Blend, Compiling, blend_state, create_stage_modules, destroy_stage_modules,
};
use rikka_gpu::error::Result;
use rikka_spirv::Shader;

const SPEC_MASKED: u32 = 0;
const SPEC_FEATURES: u32 = 1;
const SPEC_CLIPPING: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PassFormats {
    pub color: vk::Format,
    pub second: vk::Format,
    pub second_written: bool,
    pub depth: vk::Format,
    pub samples: vk::SampleCountFlags,
    pub shadow: bool,
}

impl PassFormats {
    pub(crate) fn depth_only(depth: vk::Format, samples: vk::SampleCountFlags) -> Self {
        Self {
            color: vk::Format::UNDEFINED,
            second: vk::Format::UNDEFINED,
            second_written: false,
            depth,
            samples,
            shadow: false,
        }
    }

    pub(crate) fn shadow(depth: vk::Format, samples: vk::SampleCountFlags) -> Self {
        Self {
            shadow: true,
            ..Self::depth_only(depth, samples)
        }
    }

    pub(crate) fn color(
        color: vk::Format,
        depth: vk::Format,
        samples: vk::SampleCountFlags,
    ) -> Self {
        Self {
            color,
            ..Self::depth_only(depth, samples)
        }
    }

    pub(crate) fn pair(
        color: vk::Format,
        second: vk::Format,
        samples: vk::SampleCountFlags,
    ) -> Self {
        Self {
            second,
            second_written: true,
            ..Self::color(color, vk::Format::UNDEFINED, samples)
        }
    }

    pub(crate) fn with_second(mut self, second: vk::Format, written: bool) -> Self {
        self.second = second;
        self.second_written = written;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PipelineKey {
    pub vertex: u64,
    pub fragment: Option<u64>,
    pub raster: Raster,
    pub formats: PassFormats,
    pub masked: bool,
    pub features: u32,
    pub clipping: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Raster {
    pub polygon: Polygon,
    pub blend: Blend,
    pub depth_bias: i32,
    pub depth_bias_slope: f32,
    pub sample_shading: bool,
    pub alpha_to_coverage: bool,
}

impl Raster {
    pub(crate) const FILL: Self = Self {
        polygon: Polygon::Fill,
        blend: Blend::Opaque,
        depth_bias: 0,
        depth_bias_slope: 0.0,
        sample_shading: false,
        alpha_to_coverage: false,
    };

    pub(crate) fn from_state(state: MaterialState) -> Self {
        Self {
            polygon: state.polygon,
            blend: state.blend,
            depth_bias: state.depth_bias,
            depth_bias_slope: state.depth_bias_slope,
            sample_shading: state.sample_shading,
            alpha_to_coverage: state.alpha_to_coverage,
        }
    }

    pub(crate) fn depth_of(state: MaterialState) -> Self {
        Self {
            depth_bias: state.depth_bias,
            depth_bias_slope: state.depth_bias_slope,
            ..Self::FILL
        }
    }

    pub(crate) fn blended(blend: Blend) -> Self {
        Self {
            blend,
            ..Self::FILL
        }
    }

    pub(crate) fn biased(constant: i32, slope: f32) -> Self {
        Self {
            depth_bias: constant,
            depth_bias_slope: slope,
            ..Self::FILL
        }
    }
}

impl PartialEq for Raster {
    fn eq(&self, other: &Self) -> bool {
        self.polygon == other.polygon
            && self.blend == other.blend
            && self.depth_bias == other.depth_bias
            && self.depth_bias_slope.to_bits() == other.depth_bias_slope.to_bits()
            && self.sample_shading == other.sample_shading
            && self.alpha_to_coverage == other.alpha_to_coverage
    }
}

impl Eq for Raster {}

impl std::hash::Hash for Raster {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (self.polygon, self.blend, self.depth_bias).hash(state);
        self.depth_bias_slope.to_bits().hash(state);
        (self.sample_shading, self.alpha_to_coverage).hash(state);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DrawState {
    pub cull: Cull,
    pub depth_test: bool,
    pub depth_write: bool,
    pub depth_compare: vk::CompareOp,
}

impl DrawState {
    pub(crate) const NO_DEPTH: Self = Self {
        cull: Cull::None,
        depth_test: false,
        depth_write: false,
        depth_compare: vk::CompareOp::ALWAYS,
    };

    pub(crate) fn depth_tested(cull: Cull, compare: vk::CompareOp, write: bool) -> Self {
        Self {
            cull,
            depth_test: true,
            depth_write: write,
            depth_compare: compare,
        }
    }

    pub(crate) fn from_material(state: MaterialState, compare: vk::CompareOp) -> Self {
        Self {
            cull: state.cull,
            depth_test: state.depth_test,
            depth_write: state.depth_write,
            depth_compare: compare,
        }
    }

    pub(crate) fn record(self, device: &ash::Device, cmd: vk::CommandBuffer) {
        unsafe {
            device.cmd_set_cull_mode(cmd, cull_mode(self.cull));
            device.cmd_set_depth_test_enable(cmd, self.depth_test);
            device.cmd_set_depth_write_enable(cmd, self.depth_write);
            device.cmd_set_depth_compare_op(cmd, self.depth_compare);
        }
    }
}

fn cull_mode(cull: Cull) -> vk::CullModeFlags {
    match cull {
        Cull::Back => vk::CullModeFlags::BACK,
        Cull::Front => vk::CullModeFlags::FRONT,
        Cull::None => vk::CullModeFlags::NONE,
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Recipe {
    pub key: PipelineKey,
    pub vertex: MaterialShader,
    pub fragment: Option<MaterialShader>,
    pub clips: bool,
}

impl Recipe {
    pub(crate) fn clippable(mut self) -> Self {
        self.clips = true;
        self
    }

    pub(crate) fn clipped(&self, clip_fragment: &MaterialShader) -> Recipe {
        let fragment = self
            .fragment
            .clone()
            .unwrap_or_else(|| clip_fragment.clone());
        Recipe {
            key: PipelineKey {
                fragment: Some(fragment.hash()),
                clipping: true,
                ..self.key
            },
            vertex: self.vertex.clone(),
            fragment: Some(fragment),
            clips: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Compute {
    Cull,
    Cluster,
    Pyramid,
    Skin,
    Drape,
    Pick,
}

impl Compute {
    const ALL: [Self; 6] = [
        Self::Cull,
        Self::Cluster,
        Self::Pyramid,
        Self::Skin,
        Self::Drape,
        Self::Pick,
    ];

    fn shader(self) -> Shader {
        match self {
            Self::Cull => shaders_3d::CULL_COMP,
            Self::Cluster => shaders_3d::CLUSTER_COMP,
            Self::Pyramid => shaders_3d::HZB_COMP,
            Self::Skin => shaders_3d::SKIN_COMP,
            Self::Drape => shaders_3d::DRAPE_COMP,
            Self::Pick => shaders_3d::PICK_COMP,
        }
    }
}

struct OwnedPipeline {
    device: Arc<RenderDevice>,
    pipeline: vk::Pipeline,
}

impl Drop for OwnedPipeline {
    fn drop(&mut self) {
        unsafe { self.device.handle().destroy_pipeline(self.pipeline, None) };
    }
}

pub(crate) struct MeshPipelines {
    device: Arc<RenderDevice>,
    layout: vk::PipelineLayout,
    pipelines: FxHashMap<PipelineKey, Compiling<OwnedPipeline>>,
    computes: Vec<Compiling<OwnedPipeline>>,
    kernels: FxHashMap<u64, Compiling<OwnedPipeline>>,
    material_kernels: FxHashMap<u64, Compiling<OwnedPipeline>>,
}

impl MeshPipelines {
    pub(crate) fn new(
        device: Arc<RenderDevice>,
        set_layout: vk::DescriptorSetLayout,
    ) -> Result<Self> {
        let set_layouts = [set_layout];
        let ranges = [vk::PushConstantRange::default()
            .stage_flags(
                vk::ShaderStageFlags::VERTEX
                    | vk::ShaderStageFlags::FRAGMENT
                    | vk::ShaderStageFlags::COMPUTE,
            )
            .offset(0)
            .size(crate::state::PUSH_SIZE)];
        let info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&set_layouts)
            .push_constant_ranges(&ranges);
        let layout = unsafe { device.handle().create_pipeline_layout(&info, None)? };
        let computes = Compute::ALL
            .into_iter()
            .map(|compute| {
                Compiling::start(&device, move |device| {
                    Ok(OwnedPipeline {
                        device: device.clone(),
                        pipeline: create_compute_pipeline(device, compute.shader().spirv, layout)?,
                    })
                })
            })
            .collect();
        Ok(Self {
            device,
            layout,
            pipelines: FxHashMap::default(),
            computes,
            kernels: FxHashMap::default(),
            material_kernels: FxHashMap::default(),
        })
    }

    pub(crate) fn layout(&self) -> vk::PipelineLayout {
        self.layout
    }

    pub(crate) fn prepare(&mut self, recipe: &Recipe) {
        self.compiling(recipe);
    }

    fn compiling(&mut self, recipe: &Recipe) -> &mut Compiling<OwnedPipeline> {
        let layout = self.layout;
        let device = &self.device;
        self.pipelines.entry(recipe.key).or_insert_with(|| {
            let owned = recipe.clone();
            Compiling::start(device, move |device| build_graphics(device, layout, &owned))
        })
    }

    pub(crate) fn when_ready(&mut self, recipe: &Recipe, notify: impl FnOnce() + Send + 'static) {
        self.compiling(recipe).when_ready(notify);
    }

    pub(crate) fn ready(&self, recipe: &Recipe) -> bool {
        self.pipelines
            .get(&recipe.key)
            .is_some_and(Compiling::ready)
    }

    pub(crate) fn get(&mut self, recipe: &Recipe) -> Result<vk::Pipeline> {
        Ok(self.compiling(recipe).get()?.pipeline)
    }

    pub(crate) fn prepare_kernel(&mut self, shader: Shader) {
        self.compiling_kernel(shader);
    }

    pub(crate) fn when_kernel_ready(
        &mut self,
        shader: Shader,
        notify: impl FnOnce() + Send + 'static,
    ) {
        self.compiling_kernel(shader).when_ready(notify);
    }

    fn compiling_kernel(&mut self, shader: Shader) -> &mut Compiling<OwnedPipeline> {
        let layout = self.layout;
        self.kernels.entry(shader.hash).or_insert_with(|| {
            Compiling::start(&self.device, move |device| {
                Ok(OwnedPipeline {
                    device: device.clone(),
                    pipeline: create_compute_pipeline(device, shader.spirv, layout)?,
                })
            })
        })
    }

    pub(crate) fn kernel(&mut self, shader: Shader) -> Result<vk::Pipeline> {
        Ok(self.compiling_kernel(shader).get()?.pipeline)
    }

    pub(crate) fn when_material_kernel_ready(
        &mut self,
        shader: &MaterialShader,
        notify: impl FnOnce() + Send + 'static,
    ) {
        self.compiling_material(shader).when_ready(notify);
    }

    pub(crate) fn prepare_material_kernel(&mut self, shader: &MaterialShader) {
        self.compiling_material(shader);
    }

    fn compiling_material(&mut self, shader: &MaterialShader) -> &mut Compiling<OwnedPipeline> {
        let layout = self.layout;
        self.material_kernels
            .entry(shader.hash())
            .or_insert_with(|| {
                let shader = shader.clone();
                Compiling::start(&self.device, move |device| {
                    Ok(OwnedPipeline {
                        device: device.clone(),
                        pipeline: create_compute_pipeline(device, shader.spirv(), layout)?,
                    })
                })
            })
    }

    pub(crate) fn material_kernel(&mut self, shader: &MaterialShader) -> Result<vk::Pipeline> {
        Ok(self.compiling_material(shader).get()?.pipeline)
    }

    pub(crate) fn compute(&mut self, compute: Compute) -> Result<vk::Pipeline> {
        Ok(self.computes[compute as usize].get()?.pipeline)
    }
}

impl Drop for MeshPipelines {
    fn drop(&mut self) {
        self.pipelines.clear();
        self.computes.clear();
        self.kernels.clear();
        self.material_kernels.clear();
        unsafe {
            self.device
                .handle()
                .destroy_pipeline_layout(self.layout, None)
        };
    }
}

fn build_graphics(
    device: &Arc<RenderDevice>,
    layout: vk::PipelineLayout,
    recipe: &Recipe,
) -> Result<OwnedPipeline> {
    let (vertex, fragment) = create_stage_modules(
        device,
        recipe.vertex.spirv(),
        recipe.fragment.as_ref().map(MaterialShader::spirv),
    )?;
    let pipeline = assemble(device, layout, &recipe.key, vertex, fragment);
    unsafe { destroy_stage_modules(device, vertex, fragment) };
    Ok(OwnedPipeline {
        device: device.clone(),
        pipeline: pipeline?,
    })
}

fn assemble(
    device: &RenderDevice,
    layout: vk::PipelineLayout,
    key: &PipelineKey,
    vertex: vk::ShaderModule,
    fragment: Option<vk::ShaderModule>,
) -> Result<vk::Pipeline> {
    let data = [vk::TRUE, key.features, vk::TRUE];
    let entry = |id: u32, slot: u32| {
        vk::SpecializationMapEntry::default()
            .constant_id(id)
            .offset(slot * size_of::<u32>() as u32)
            .size(size_of::<u32>())
    };
    let mut entries = Vec::with_capacity(data.len());
    if key.masked {
        entries.push(entry(SPEC_MASKED, 0));
    }
    if key.features != ALL_FEATURES {
        entries.push(entry(SPEC_FEATURES, 1));
    }
    if key.clipping {
        entries.push(entry(SPEC_CLIPPING, 2));
    }
    let used = entries.as_slice();
    let specialization = vk::SpecializationInfo::default()
        .map_entries(used)
        .data(bytemuck::cast_slice(&data));
    let stage = |stage: vk::ShaderStageFlags, module: vk::ShaderModule| {
        let info = vk::PipelineShaderStageCreateInfo::default()
            .stage(stage)
            .module(module)
            .name(c"main");
        match used.is_empty() {
            false => info.specialization_info(&specialization),
            true => info,
        }
    };
    let mut stages = vec![stage(vk::ShaderStageFlags::VERTEX, vertex)];
    if let Some(fragment) = fragment {
        stages.push(stage(vk::ShaderStageFlags::FRAGMENT, fragment));
    }

    let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
    let viewport = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);

    let rasterization = rasterization_state(key);
    let multisample = multisample_state(key);
    let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default();
    let (color_formats, attachments) = color_attachments(key);
    let color_blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&attachments);

    let dynamic_states = [
        vk::DynamicState::VIEWPORT,
        vk::DynamicState::SCISSOR,
        vk::DynamicState::CULL_MODE,
        vk::DynamicState::DEPTH_TEST_ENABLE,
        vk::DynamicState::DEPTH_WRITE_ENABLE,
        vk::DynamicState::DEPTH_COMPARE_OP,
    ];
    let dynamic = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

    let mut rendering = vk::PipelineRenderingCreateInfo::default()
        .color_attachment_formats(&color_formats)
        .depth_attachment_format(key.formats.depth);

    let info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vertex_input)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport)
        .rasterization_state(&rasterization)
        .multisample_state(&multisample)
        .depth_stencil_state(&depth_stencil)
        .color_blend_state(&color_blend)
        .dynamic_state(&dynamic)
        .layout(layout)
        .push_next(&mut rendering);

    let pipelines = unsafe {
        device
            .handle()
            .create_graphics_pipelines(device.pipeline_cache(), &[info], None)
    };
    match pipelines {
        Ok(pipelines) => Ok(pipelines[0]),
        Err((_, err)) => Err(err.into()),
    }
}

fn rasterization_state(key: &PipelineKey) -> vk::PipelineRasterizationStateCreateInfo<'static> {
    let raster = key.raster;
    let mut rasterization = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(match raster.polygon {
            Polygon::Fill => vk::PolygonMode::FILL,
            Polygon::Line => vk::PolygonMode::LINE,
            Polygon::Point => vk::PolygonMode::POINT,
        })
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .line_width(1.0);
    if raster.depth_bias != 0 || raster.depth_bias_slope != 0.0 {
        rasterization = rasterization
            .depth_bias_enable(true)
            .depth_bias_constant_factor(raster.depth_bias as f32)
            .depth_bias_slope_factor(raster.depth_bias_slope);
    }
    if key.formats.shadow {
        rasterization = rasterization.depth_clamp_enable(true);
    }
    rasterization
}

fn multisample_state(key: &PipelineKey) -> vk::PipelineMultisampleStateCreateInfo<'static> {
    let multisample = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(key.formats.samples)
        .alpha_to_coverage_enable(key.raster.alpha_to_coverage);
    match key.raster.sample_shading {
        true => multisample
            .sample_shading_enable(true)
            .min_sample_shading(1.0),
        false => multisample,
    }
}

fn color_attachments(
    key: &PipelineKey,
) -> (Vec<vk::Format>, Vec<vk::PipelineColorBlendAttachmentState>) {
    let formats: Vec<vk::Format> = [key.formats.color, key.formats.second]
        .into_iter()
        .filter(|format| *format != vk::Format::UNDEFINED)
        .collect();
    let attachments = formats
        .iter()
        .enumerate()
        .map(|(index, format)| match *format {
            _ if index == 1 && !key.formats.second_written => {
                vk::PipelineColorBlendAttachmentState::default()
                    .blend_enable(false)
                    .color_write_mask(vk::ColorComponentFlags::empty())
            }
            SURFACE_FORMAT => vk::PipelineColorBlendAttachmentState::default()
                .blend_enable(false)
                .color_write_mask(vk::ColorComponentFlags::RGBA),
            IDENTITY_FORMAT | OUTLINE_FORMAT => vk::PipelineColorBlendAttachmentState::default()
                .blend_enable(false)
                .color_write_mask(vk::ColorComponentFlags::R),
            _ => blend_state(key.raster.blend),
        })
        .collect();
    (formats, attachments)
}
