mod chain;
mod characters;
pub(crate) mod command;
mod compose;
mod cull;
mod deform;
mod depth;
mod direct;
mod history;
mod ltc;
mod overlay;
mod shade;
mod shadow;
pub(crate) mod stage;
mod surfaces;
mod workbench;

use std::hash::Hasher;

use ash::vk;
use glam::Mat4;
use rustc_hash::FxHasher;

use crate::build::{self, BuiltFrame};
use crate::composite::{CompositePlan, NodeOutput};
use crate::compute_inputs::ComputeInputs;
use crate::draw::Frame;
use crate::error::{Error, Result};
use crate::frame_data::{FrameAddresses, FrameWrite, cluster_stride};
use crate::gpu::{CLUSTER_SLICES, CLUSTER_TILE, GpuView, NONE_INDEX};
use crate::graph::{self, BoundImage, Extents, FrameGraph, Import, Imports, SHADER_STAGES};
use crate::light::ShadowQuality;
use crate::look::Look;
use crate::look::gpu::{GpuLook, TRACED_LIGHTS, split};
use crate::look::resolve::Resolved;
use crate::look::tier::{Mode, Tiers};
use crate::material::MaterialPool;
use crate::overlay::OverlayFrame;
use crate::ray;
use crate::shader::Tracing;
use crate::shadow::{Allocators, DEFAULT_SHADOW_DISTANCE, Plan, PlanInput, ShadowAtlas, Traced};
use crate::space::EmitterFrame;
use crate::state::Renderer3dState;
use crate::temporal::TemporalFrame;
use crate::viewport::{Viewport, Warming};
use rikka_gpu::core::barrier::MemoryAccess;
use rikka_gpu::core::convert::YcbcrConverter;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::image::{GpuImage, record_barriers};
use rikka_gpu::core::sampler::Samplers;
use rikka_gpu::error::Store;
use rikka_gpu::texture::TexturePool;

pub(crate) use chain::{Chain, Written};
pub(crate) use compose::OutlineMask;
pub(crate) use cull::draws_read;
pub(crate) use stage::{AtlasImports, Guides, PostSurface, SceneImages, Stage, ViewFacts};
pub(crate) use surfaces::{LAYERS_FORMAT, SURFACE_FORMAT, surface_formats};

use command::{BindState, bind_table};
use history::HistoryCopy;
use surfaces::deform_signature;

pub(crate) const PASS_MASKED: u32 = 1 << 0;
pub(crate) const PASS_SHADOW: u32 = 1 << 1;
pub(crate) const PASS_LATE: u32 = 1 << 2;
pub(crate) const PASS_SEE_THROUGH: u32 = 1 << 3;

const SHADOW_NOISE_PERIOD: u32 = 64;

#[derive(Clone, Copy)]
struct LightCounts {
    directional: u32,
    listed: u32,
    stride: u32,
}

#[derive(Default)]
pub(crate) struct FrameScratch {
    composite: CompositePlan,
    outputs: Vec<Option<NodeOutput>>,
    emitters: EmitterFrame,
    resolved: Resolved,
    plan: Plan,
    overlays: OverlayFrame,
}

pub(crate) struct Record<'a> {
    pub(crate) mesh: &'a mut Renderer3dState,
    pub(crate) textures: &'a mut TexturePool,
    pub(crate) convert: &'a mut YcbcrConverter,
    bound: BindState,
    clipping: bool,
}

impl<'a> Record<'a> {
    pub(crate) fn new(
        mesh: &'a mut Renderer3dState,
        textures: &'a mut TexturePool,
        convert: &'a mut YcbcrConverter,
    ) -> Self {
        Self {
            mesh,
            textures,
            convert,
            bound: BindState::default(),
            clipping: false,
        }
    }

    pub(crate) fn frame(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        viewport: &mut Viewport,
        frame: &Frame,
    ) -> Result<()> {
        self.clipping = frame.editor.clips();
        self.textures.flush(ctx, self.convert)?;
        self.mesh.flush(ctx)?;
        self.forget_bindings();
        viewport.retarget(ctx.completed_value);
        let scale = viewport.render_scale();
        if let Some(temporal) = viewport.temporal.as_mut() {
            temporal.begin_timing(ctx);
        }

        let deferring = viewport.config.warming == Warming::DrawPlain;
        let look = frame
            .look
            .as_ref()
            .filter(|look| !deferring || self.mesh.look_ready(look));
        viewport.look_pending = frame.look.is_some() && look.is_none();
        let mut built = std::mem::take(&mut self.mesh.built);
        let result = self.record_frame(ctx, viewport, frame, look, &mut built);
        self.mesh.built = built;
        if let Some(temporal) = viewport.temporal.as_mut() {
            temporal.end_timing(ctx, scale);
        }
        record_barriers(ctx.device, ctx.cmd, &self.textures.release_barriers());
        result
    }

    fn record_frame(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        viewport: &mut Viewport,
        frame: &Frame,
        look: Option<&Look>,
        built: &mut BuiltFrame,
    ) -> Result<()> {
        let mut scratch = std::mem::take(&mut self.mesh.frame_scratch);
        let result = self.record_scratch_frame(ctx, viewport, frame, look, built, &mut scratch);
        self.mesh.frame_scratch = scratch;
        result
    }

    fn record_scratch_frame(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        viewport: &mut Viewport,
        frame: &Frame,
        look: Option<&Look>,
        built: &mut BuiltFrame,
        scratch: &mut FrameScratch,
    ) -> Result<()> {
        let FrameScratch {
            composite,
            outputs,
            emitters,
            resolved,
            plan,
            overlays,
        } = scratch;
        let tiers = match look {
            Some(look) => look.tiers(self.mesh.rays.is_some())?,
            None => Tiers::NONE,
        };
        composite.resolve(frame, look, &self.mesh.effects, self.textures)?;
        emitters.resolve(look, frame.camera.position(), frame.time, self.textures)?;
        overlays.resolve(&frame.overlays, &self.mesh.overlays)?;
        let overlay_batches = self
            .mesh
            .frames
            .write_overlays(ctx, &frame.overlays, overlays)?;
        let composite = &*composite;
        let emitters = &*emitters;
        let overlays = &*overlays;
        self.build_frame(ctx, frame, look, built, composite, emitters)?;
        let mut gpu_look = resolved.resolve(frame, look);
        if tiers.shadows == Mode::Off {
            let studio = resolved.studio_shadow;
            for (index, light) in resolved.lights.iter_mut().enumerate() {
                if studio != Some(index) {
                    light.shadow = ShadowQuality::None;
                }
            }
            for key in &mut resolved.keys {
                key.light.shadow = ShadowQuality::None;
            }
        }
        let traced = match (tiers.shadows, frame.fluids.is_empty()) {
            (Mode::RayTraced, true) => {
                Traced::of(&resolved.lights, resolved.leading, &resolved.keys)
            }
            _ => Traced::NONE,
        };
        let input = PlanInput {
            camera: &frame.camera,
            distance: look.map_or(DEFAULT_SHADOW_DISTANCE, |look| look.shadows.distance),
            lights: &resolved.lights,
            leading: resolved.leading,
            keys: &resolved.keys,
            figures: &frame.figures,
            bounds: &built.figure_bounds,
            instances: &built.instances,
            cacheable: frame.fluids.is_empty(),
            tinted: built.tinted_casters,
            traced,
        };
        let shadowed = self.plan_shadows(ctx, &input, plan)?;
        let counts = build::write_lights(built, resolved, plan, &self.mesh.profiles)?;
        let shadow = self.mesh.shadow.as_ref();
        built.shadows.extend(
            plan.views
                .iter()
                .map(|view| view.to_gpu(shadow.map_or(1, |atlas| atlas.side_of(view.atlas)))),
        );
        built
            .bones
            .extend(frame.bones.iter().map(Mat4::to_cols_array));
        build::describe_buckets(built, plan.views.len() as u32, viewport.hzb_ref().is_some());
        check_draw_counts(ctx, built)?;
        let traced_lights = match tiers.shadows {
            Mode::RayTraced => traced_lights(built),
            Mode::ScreenSpace | Mode::Off => [NONE_INDEX; TRACED_LIGHTS],
        };
        let traces = tiers.traces_scene(traced_lights.iter().any(|&light| light != NONE_INDEX));
        if traces {
            let rays = self.ray_prepare(ctx, frame, built)?;
            let trace = &mut gpu_look.trace;
            trace.scene = split(rays.scene);
            trace.parts = split(rays.parts);
            trace.indices16 = split(rays.indices16);
            trace.indices32 = split(rays.indices32);
            trace.hooks = split(rays.hooks);
            for (slot, &light) in trace.shadow_lights.iter_mut().zip(&traced_lights) {
                *slot = built
                    .lights
                    .get(light as usize)
                    .map_or(NONE_INDEX, |light| light.shadow);
            }
            trace.lights = traced_lights;
            trace.shadow_samples = look.map_or(1, |look| ray::samples(look.shadows.samples));
        }
        gpu_look.trace.frame = viewport.history.frames as u32;

        let signature = deform_signature(built);
        let view_projection = frame.camera.view_projection();
        let cluster_count = viewport.cluster_count();
        let local = counts.listed - counts.directional;
        let stride = cluster_stride(cluster_count, local);
        self.mesh.warn_cluster_overflow(local, stride);
        let mut view = Self::view_constants(
            viewport,
            frame,
            self.mesh.shadow.as_ref().filter(|_| shadowed),
            built,
            LightCounts {
                directional: counts.directional,
                listed: counts.listed,
                stride,
            },
        );
        let display = viewport.display_extent();
        let editor = frame.editor.resolve(&frame.camera, display.width);
        let grid = editor.as_ref().and_then(|editor| editor.grid);
        let overlay_view =
            overlay::active(built, overlays, grid).then(|| display_view(&view, display));
        let content = content_signature(
            &view,
            built,
            frame,
            &gpu_look,
            [self.mesh.materials.revision(), self.mesh.meshes.revision()],
            emitters,
        );
        let extent = viewport.extent();
        let temporal = viewport
            .temporal
            .as_mut()
            .and_then(|state| state.advance(extent, content, frame.cut));
        if let Some(temporal) = temporal {
            jitter_view(&mut view, frame, temporal, viewport.history.frames);
            view.cut = u32::from(temporal.reset);
        }
        let previous_positions = self.prepare_history(
            viewport,
            view_projection,
            signature,
            frame,
            look,
            &mut gpu_look,
        );
        let history = self.reserve_history(
            ctx,
            viewport,
            look.is_some(),
            composite.reads().motion,
            built.deform_vertices,
        )?;

        let built = &*built;
        let views = &plan.views[..];
        let figures = &resolved.figures;
        let mut facts = ViewFacts::of(viewport, ctx.device.depth_resolve_mode());
        facts.formats.emission = composite.reads().emission;
        if traced_lights.iter().any(|&light| light != NONE_INDEX) {
            facts.formats.tracing = Tracing::RayQueries;
        }
        let stage = Stage {
            frame,
            built,
            look,
            shadows: views,
            leading: resolved
                .leading
                .map(|leading| resolved.lights[leading].kind.direction()),
            style: resolved.style,
            view: facts,
            clustered: stride != 0,
            composite,
            temporal,
            tiers,
            traces,
            traced_lights,
            emitters,
            overlays,
            grid,
        };
        let scratch = std::mem::take(&mut self.mesh.graph_scratch);
        let render = viewport.extent();
        let extents = Extents {
            render,
            previous: temporal.map_or(render, |temporal| temporal.previous),
            capacity: viewport.capacity(),
        };
        let mut graph = FrameGraph::new(extents, self.mesh.rays.is_some(), scratch);
        let scene = self.declare(&mut graph, stage, history, outputs)?;
        let imports = imports(viewport, self.mesh, self.textures, ctx.samplers);
        viewport
            .graph
            .realize(&graph, ctx, &mut self.mesh.look_state.images, &imports)?;
        scene.publish(&viewport.graph, &mut gpu_look);
        let raster = Mat4::from_cols_array(&view.view_projection);
        let fluid = self.fluid_prepare(ctx, viewport, stage, &scene, raster)?;

        let addresses = self.mesh.frames.write(
            ctx,
            &FrameWrite {
                view: &view,
                instances: &built.instances,
                lights: &built.lights,
                shadows: &built.shadows,
                bones: &built.bones,
                morph_weights: &frame.morph_weights,
                materials: self.mesh.materials.address(),
                blocks: self.mesh.materials.blocks_address(),
                buckets: &built.bucket_table,
                profiles: self.mesh.profiles.address(),
                cluster_count,
                cluster_stride: stride,
                look: &gpu_look,
                figures,
                environment: self.mesh.look_state.environment.device_address(),
                previous_positions,
                fluid: fluid.as_ref().map_or(0, |fluid| fluid.address),
                previous_transforms: viewport
                    .history
                    .previous_transforms(&built.instances, !frame.cut),
                overlay_view: overlay_view.as_ref(),
                editor: editor.as_ref().map(|editor| &editor.gpu),
            },
        )?;
        let addresses = FrameAddresses {
            effects: self.mesh.frames.write_effects(ctx, composite.blocks())?,
            emitters: self.mesh.frames.write_emitters(ctx, emitters)?,
            overlay_batches,
            ..addresses
        };
        bind_table(ctx, self.mesh.pipelines.layout());
        ctx.memory_barrier(
            MemoryAccess::new(
                vk::PipelineStageFlags2::TRANSFER,
                vk::AccessFlags2::TRANSFER_WRITE,
            ),
            self.uploads_read(),
        );

        self.mesh.graph_scratch =
            graph::execute(graph, self, ctx, viewport, &addresses, fluid.as_ref())?;
        viewport.history.remember_transforms(&built.instances);
        self.finish_history(viewport, frame, look, view_projection, signature);
        Ok(())
    }

    fn declare<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        history: Option<HistoryCopy>,
        outputs: &mut Vec<Option<NodeOutput>>,
    ) -> Result<SceneImages> {
        let atlas = self.mesh.shadow.as_ref();
        let atlases = AtlasImports {
            main: atlas.is_some(),
            figures: atlas.is_some_and(|atlas| atlas.figures.is_some()),
            tinted: stage.built.tinted_casters && atlas.is_some_and(|atlas| atlas.tint.is_some()),
        };
        let mut scene = SceneImages::import(graph, stage.view, atlases);
        self.declare_pbr_lut(graph, &mut scene);
        self.declare_ltc(graph, &mut scene);
        self.declare_geometry(graph, stage, &scene);
        let tiers = stage.tiers;
        if stage.traces {
            self.declare_ray_build(graph);
        }
        let surfaced = stage.look.is_some()
            || !stage.frame.fluids.is_empty()
            || stage.temporal.is_some()
            || stage.frame.editor.workbench.composites();
        if surfaced {
            self.declare_surfaces(graph, stage, &mut scene);
            self.declare_characters(graph, stage, &mut scene);
        }
        if let Some(look) = stage.look {
            self.declare_atmosphere(graph, stage, look, &mut scene)?;
            if stage.traces || tiers.illumination != Mode::Off {
                self.declare_ray_guides(graph, &mut scene);
            }
            match tiers.occlusion {
                Mode::RayTraced => self.declare_ray_occlusion(graph, look, &mut scene),
                Mode::ScreenSpace | Mode::Off => {
                    self.declare_occlusion(graph, stage, look, &mut scene)
                }
            }
        }
        self.declare_shadow(graph, stage, &scene);
        if let Some(look) = stage.look {
            if tiers.shadows == Mode::RayTraced {
                self.declare_ray_shadows(graph, look, &mut scene, stage.traced_lights);
            }
            if tiers.illumination != Mode::Off {
                self.declare_ray_illumination(graph, look, tiers.illumination, &mut scene);
            }
            self.declare_fog_volume(graph, stage, look, &mut scene);
            self.declare_space_emitters(graph, stage, &scene);
        }
        let mut chain = self.declare_shade(graph, stage, &mut scene);
        self.declare_space_emitter_targets(graph, stage, &mut scene);
        if !stage.frame.fluids.is_empty() {
            if stage.temporal.is_some() && scene.opaque.is_none() {
                scene.opaque = Some(chain.current());
            }
            self.declare_fluid(graph, stage, &mut scene, &mut chain);
        }
        if scene.opaque.is_some() {
            scene.blended = Some(chain.current());
        }
        if stage.look.is_some() {
            self.declare_ray_keep(graph, &scene, &chain);
        }
        if stage.look.is_none() && stage.frame.editor.workbench.composites() {
            self.declare_workbench(graph, stage, &scene, &mut chain);
        }
        self.declare_compose(graph, stage, &mut scene, chain, outputs);
        self.declare_history(graph, history);
        Ok(scene)
    }

    fn uploads_read(&self) -> MemoryAccess {
        let (built, read) = match self.mesh.rays.is_some() {
            true => (
                vk::PipelineStageFlags2::ACCELERATION_STRUCTURE_BUILD_KHR,
                vk::AccessFlags2::SHADER_READ,
            ),
            false => (vk::PipelineStageFlags2::NONE, vk::AccessFlags2::NONE),
        };
        MemoryAccess::new(
            vk::PipelineStageFlags2::DRAW_INDIRECT
                | vk::PipelineStageFlags2::INDEX_INPUT
                | SHADER_STAGES
                | built,
            vk::AccessFlags2::INDIRECT_COMMAND_READ
                | vk::AccessFlags2::INDEX_READ
                | vk::AccessFlags2::SHADER_STORAGE_READ
                | vk::AccessFlags2::SHADER_STORAGE_WRITE
                | read,
        )
    }

    fn build_frame(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        frame: &Frame,
        look: Option<&Look>,
        built: &mut BuiltFrame,
        composite: &CompositePlan,
        emitters: &EmitterFrame,
    ) -> Result<()> {
        let compute_inputs = ComputeInputs::collect(frame, ctx.device)?;
        ctx.waits_on(compute_inputs.waits());
        if !compute_inputs.buffers.is_empty() {
            ctx.retire
                .defer(ctx.frame_value, Box::new(compute_inputs.buffers));
        }
        build::build(built, frame, look, &self.mesh.meshes, &self.mesh.materials)?;
        self.textures.take_over(
            ctx,
            sampled_slots(built, &self.mesh.materials)
                .chain(composite.texture_slots())
                .chain(emitters.texture_slots()),
        );
        let deform = self.mesh.frames.ensure_deform(ctx, built.deform_vertices)?;
        build::apply_deform_addresses(built, deform);
        Ok(())
    }

    fn plan_shadows(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        input: &PlanInput<'_>,
        plan: &mut Plan,
    ) -> Result<bool> {
        let planned = plan.size(input);
        if planned.side == 0 {
            return Ok(false);
        }
        self.mesh.ensure_shadow_atlas(ctx, planned, input.tinted)?;
        let atlas = self.mesh.shadow.as_mut().expect("shadow atlas exists");
        let mut generations = FxHasher::default();
        generations.write_u64(atlas.generation);
        generations.write_u64(self.mesh.meshes.revision());
        let generation = generations.finish();
        plan.place(
            input,
            Allocators {
                main: &mut atlas.allocator,
                figures: atlas.figures.as_mut().map(|figures| &mut figures.allocator),
            },
            &mut atlas.cache,
            generation,
        );
        Ok(true)
    }

    fn view_constants(
        viewport: &Viewport,
        frame: &Frame,
        shadow: Option<&ShadowAtlas>,
        built: &BuiltFrame,
        lit: LightCounts,
    ) -> GpuView {
        let camera = &frame.camera;
        let view_projection = camera.view_projection();
        let near = camera.near();
        let extent = viewport.extent();
        let capacity = viewport.capacity();

        let far = DEFAULT_SHADOW_DISTANCE.max(near * 2.0);
        let scale = CLUSTER_SLICES as f32 / (far / near).log2();
        let bias = -(near.log2() * scale);

        let (hzb_image, hzb_size, hzb_levels) = match viewport.hzb_ref() {
            Some(hzb) if viewport.drawn => (
                hzb.sampled_slot,
                [hzb.extent.width as f32, hzb.extent.height as f32],
                hzb.levels,
            ),
            _ => (NONE_INDEX, [1.0, 1.0], 1),
        };
        let tint = shadow
            .filter(|_| built.tinted_casters)
            .and_then(|atlas| atlas.tint.as_ref());

        GpuView {
            view: camera.view.to_cols_array(),
            projection: camera.projection.to_cols_array(),
            view_projection: view_projection.to_cols_array(),
            inverse_view_projection: view_projection.inverse().to_cols_array(),
            inverse_projection: camera.projection.inverse().to_cols_array(),
            frustum: camera.frustum_planes().map(|plane| plane.to_array()),
            camera_position: camera.position().to_array(),
            near,
            camera_forward: camera.forward().to_array(),
            viewport: [extent.width as f32, extent.height as f32],
            inverse_viewport: [1.0 / extent.width as f32, 1.0 / extent.height as f32],
            cluster_counts: viewport.targets().cluster_counts,
            cluster_tile: CLUSTER_TILE,
            cluster_depth_scale: scale,
            cluster_depth_bias: bias,
            light_count: lit.listed,
            directional_count: lit.directional,
            instance_count: built.instances.len() as u32,
            hzb_size,
            hzb_levels,
            exposure: frame.exposure,
            ambient: frame.ambient.to_array(),
            time: frame.time,
            shadow_atlas: shadow.map_or(NONE_INDEX, |atlas| atlas.slot),
            hzb_image,
            cluster_stride: lit.stride,
            shadow_classes: built.layout.shadow_classes,
            shadow_tint: tint.map_or(NONE_INDEX, |tint| tint.color.slot),
            shadow_tint_depth: tint.map_or(NONE_INDEX, |tint| tint.depth.slot),
            shadow_atlas_size: shadow.map_or(0.0, |atlas| atlas.side as f32),
            jitter: [0.0; 2],
            mip_bias: 0.0,
            render_scale: 1.0,
            previous_jitter: [0.0; 2],
            cut: u32::from(frame.cut),
            shadow_noise: 0,
            figure_atlas: shadow
                .and_then(|atlas| atlas.figures.as_ref())
                .map_or(NONE_INDEX, |figures| figures.image.slot),
            capacity: [capacity.width as f32, capacity.height as f32],
            previous_viewport: [extent.width as f32, extent.height as f32],
        }
    }
}

fn display_view(view: &GpuView, display: vk::Extent2D) -> GpuView {
    let size = [display.width as f32, display.height as f32];
    GpuView {
        viewport: size,
        inverse_viewport: [1.0 / size[0], 1.0 / size[1]],
        capacity: size,
        previous_viewport: size,
        hzb_image: NONE_INDEX,
        ..*view
    }
}

fn jitter_view(view: &mut GpuView, frame: &Frame, temporal: TemporalFrame, frames: u64) {
    let camera = &frame.camera;
    let offset = Mat4::from_translation(temporal.jitter_ndc.extend(0.0));
    let projection = offset * camera.projection;
    let view_projection = projection * camera.view;
    view.projection = projection.to_cols_array();
    view.view_projection = view_projection.to_cols_array();
    view.inverse_view_projection = view_projection.inverse().to_cols_array();
    view.inverse_projection = projection.inverse().to_cols_array();
    view.jitter = temporal.jitter_ndc.to_array();
    view.mip_bias = temporal.mip_bias;
    view.render_scale = temporal.ratio().x;
    view.previous_jitter = temporal.previous_jitter_ndc.to_array();
    view.previous_viewport = [
        temporal.previous.width as f32,
        temporal.previous.height as f32,
    ];
    view.shadow_noise = match temporal.settled {
        true => 0,
        false => (frames as u32 % SHADOW_NOISE_PERIOD) + 1,
    };
}

fn content_signature(
    view: &GpuView,
    built: &BuiltFrame,
    frame: &Frame,
    look: &GpuLook,
    revisions: [u64; 2],
    emitters: &EmitterFrame,
) -> u64 {
    let mut hasher = FxHasher::default();
    let unsized_view = GpuView {
        viewport: [0.0; 2],
        inverse_viewport: [0.0; 2],
        cluster_counts: [0; 3],
        previous_viewport: [0.0; 2],
        ..*view
    };
    hasher.write(bytemuck::bytes_of(&unsized_view));
    hasher.write(bytemuck::cast_slice(&built.instances));
    hasher.write(bytemuck::cast_slice(&built.lights));
    hasher.write(bytemuck::cast_slice(&built.bones));
    hasher.write(bytemuck::cast_slice(&frame.morph_weights));
    let mut stable = *look;
    stable.trace.scene = [0; 2];
    stable.trace.parts = [0; 2];
    stable.trace.indices16 = [0; 2];
    stable.trace.indices32 = [0; 2];
    stable.trace.frame = 0;
    hasher.write(bytemuck::bytes_of(&stable));
    for revision in revisions {
        hasher.write_u64(revision);
    }
    emitters.hash_into(&mut hasher);
    let tickets = frame
        .wets
        .iter()
        .chain(&frame.drapes)
        .map(|source| source.ready.value)
        .chain(frame.fluids.iter().map(|layer| layer.ready.value));
    for value in tickets {
        hasher.write_u64(value);
    }
    hasher.finish()
}

fn imports(
    viewport: &Viewport,
    mesh: &Renderer3dState,
    textures: &TexturePool,
    samplers: &Samplers,
) -> Imports {
    let mut imports = Imports::default();
    let targets = viewport.targets();
    let images = &targets.images;
    let render = targets.extent;
    let mut set = |import: Import, bound: BoundImage| imports.bound[import.index()] = Some(bound);
    let target = |image: &GpuImage, slot: u32, sampler: vk::Sampler| {
        BoundImage::imported(image, slot)
            .with_sampler(sampler)
            .with_extent(render)
    };
    set(
        Import::ColorTarget,
        target(
            images.color.as_ref().unwrap_or(&images.resolved),
            NONE_INDEX,
            vk::Sampler::null(),
        ),
    );
    set(
        Import::Resolved,
        target(
            &images.resolved,
            targets.resolved_slot,
            samplers.linear_clamp(),
        ),
    );
    set(
        Import::DepthTarget,
        target(
            images.depth.as_ref().unwrap_or(&images.depth_resolved),
            NONE_INDEX,
            vk::Sampler::null(),
        ),
    );
    set(
        Import::DepthResolved,
        target(
            &images.depth_resolved,
            targets.depth_resolved_slot,
            samplers.depth_reduce(),
        ),
    );
    if let Some(identity) = images
        .identity
        .as_ref()
        .or(images.identity_resolved.as_ref())
    {
        set(
            Import::IdentityTarget,
            target(identity, NONE_INDEX, vk::Sampler::null()),
        );
    }
    if let Some(identity) = &images.identity_resolved {
        set(
            Import::Identity,
            target(identity, targets.identity_slot, samplers.nearest_clamp()),
        );
    }
    if let Some(outline) = &images.outline {
        set(
            Import::Outline,
            target(outline, targets.outline_slot, samplers.nearest_clamp()),
        );
    }
    if let Some(hzb) = &images.hzb {
        set(
            Import::Hzb,
            BoundImage::imported(&hzb.image, hzb.sampled_slot).storing(),
        );
    }
    if let Some(output) = textures.image(viewport.output) {
        set(Import::Output, BoundImage::imported(output, NONE_INDEX));
    }
    if let Some(atlas) = &mesh.shadow {
        set(
            Import::ShadowAtlas,
            BoundImage::imported(&atlas.image, atlas.slot),
        );
        if let Some(tint) = &atlas.tint {
            set(
                Import::ShadowTint,
                BoundImage::imported(&tint.color.image, tint.color.slot),
            );
            set(
                Import::ShadowTintDepth,
                BoundImage::imported(&tint.depth.image, tint.depth.slot),
            );
        }
        if let Some(figures) = &atlas.figures {
            set(
                Import::FigureAtlas,
                BoundImage::imported(&figures.image.image, figures.image.slot),
            );
        }
    }
    imports
}

fn sampled_slots<'b>(
    built: &'b BuiltFrame,
    materials: &'b MaterialPool,
) -> impl Iterator<Item = u32> + 'b {
    let mut seen = rustc_hash::FxHashSet::default();
    built
        .instances
        .iter()
        .map(|instance| instance.material)
        .filter(move |&material| seen.insert(material))
        .flat_map(|material| materials.texture_slots(material))
}

fn check_draw_counts(ctx: &GpuFrameCtx<'_>, built: &BuiltFrame) -> Result<()> {
    let limit = ctx.device.limits().max_draw_indirect_count;
    match built
        .bucket_table
        .iter()
        .find(|bucket| bucket.capacity > limit)
    {
        Some(_) => Err(Error::full(Store::IndirectDraws { draws: limit })),
        None => Ok(()),
    }
}

fn traced_lights(built: &BuiltFrame) -> [u32; TRACED_LIGHTS] {
    let mut traced = [NONE_INDEX; TRACED_LIGHTS];
    let mut count = 0;
    for (index, light) in built.lights.iter().enumerate() {
        if count == TRACED_LIGHTS || light.shadow == NONE_INDEX {
            continue;
        }
        let shares = traced[..count]
            .iter()
            .any(|&other| built.lights[other as usize].shadow == light.shadow);
        if !shares {
            traced[count] = index as u32;
            count += 1;
        }
    }
    traced
}
