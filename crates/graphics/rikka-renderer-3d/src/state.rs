use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rustc_hash::{FxHashMap, FxHashSet};

use ash::vk;

use crate::build::BuiltFrame;
use crate::draw::Frame;
use crate::effect::{Effect, EffectId, EffectPool};
use crate::error::{Error, Result};
use crate::frame_data::FrameData;
use crate::graph::GraphScratch;
use crate::light::LightProfilePool;
use crate::look::Look;
use crate::look::tier::{Mode, Tiers};
use crate::look_state::LookState;
use crate::material::features::Routing;
use crate::material::{
    MaterialDesc, MaterialId, MaterialPass, MaterialPool, MaterialState, MaterialTemplate, Program,
    Value, features,
};
use crate::mesh::{Indices, MeshData, MeshId, MeshUpdate, Submesh};
use crate::overlay::OverlayPool;
use crate::pipeline::{MeshPipelines, PipelineKey, Recipe};
use crate::pool::MeshPool;
use crate::ray::{self, RayScene};
use crate::recipes::{self, ViewFormats};
use crate::record::{FrameScratch, surface_formats};
use crate::shader::{MaterialShader, Tracing};
use crate::shadow::{self, ShadowAtlas};
use crate::temporal::TemporalState;
use crate::viewport::{Targets, Viewport, ViewportConfig};
use crate::warmup::Warmup;
use crate::{fluid, pbr, post, sky, space, temporal};
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::runtime::BindPoint;
use rikka_gpu::reclaim::Reclaimer;
use rikka_gpu::texture::TextureId;
use rikka_shader_programs::Variant;
use rikka_spirv::Shader;

const EFFECT_KERNELS: [Shader; 2] = [
    shaders_3d::EFFECT_SURFACE_COMP,
    shaders_3d::EFFECT_DEPTH_COMP,
];

pub(crate) const PUSH_SIZE: u32 = 128;

pub(crate) struct Builtin {
    pub mesh_vert: MaterialShader,
    pub depth_vert: MaterialShader,
    pub depth_frag: MaterialShader,
    pub fullscreen_vert: MaterialShader,
    pub sky_frag: MaterialShader,
    pub clip_bounds_frag: MaterialShader,
    pub grid_frag: MaterialShader,
    pub outline_frag: MaterialShader,
    pub clip_frag: MaterialShader,
    pub outline_id_frag: MaterialShader,
    pub outline_edges: [MaterialShader; 2],
    pub outline_detect_frag: MaterialShader,
    pub outline_resolve_frag: MaterialShader,
    pub tonemap_frag: MaterialShader,
    pub copy_frag: MaterialShader,
    pub overlay_lines: [MaterialShader; 2],
    pub overlay_points: [MaterialShader; 2],
    pub overlay_triangles: [MaterialShader; 2],
    pub editor_grid: [MaterialShader; 2],
    pub workbench_frag: MaterialShader,
}

pub(crate) struct Renderer3dState {
    device: Arc<RenderDevice>,
    pub(crate) pipelines: MeshPipelines,
    pub(crate) meshes: MeshPool,
    pub(crate) overlays: OverlayPool,
    pub(crate) materials: MaterialPool,
    pub(crate) frames: FrameData,
    pub(crate) profiles: LightProfilePool,
    pub(crate) shadow: Option<ShadowAtlas>,
    pub(crate) builtin: Builtin,
    pub(crate) built: BuiltFrame,
    pub(crate) look_state: LookState,
    pub(crate) effects: EffectPool,
    pub(crate) frame_scratch: FrameScratch,
    pub(crate) graph_scratch: GraphScratch,
    warned_cluster_overflow: bool,
    views: Vec<ViewFormats>,
    warmed_look: Warmup,
    warmed_fluid: bool,
    warmed_denoise: Warmup,
    warmed_rays: Warmup,
    warmed_traced_shading: bool,
    warmed_overlays: bool,
    watched: FxHashMap<u64, FxHashSet<PipelineKey>>,
    pub(crate) rays: Option<RayScene>,
}

impl Renderer3dState {
    pub(crate) fn new(
        device: Arc<RenderDevice>,
        table_layout: vk::DescriptorSetLayout,
    ) -> Result<Self> {
        let pipelines = MeshPipelines::new(device.clone(), table_layout)?;
        let mut renderer = Self {
            pipelines,
            meshes: MeshPool::new(device.clone())?,
            overlays: OverlayPool::new(&device)?,
            materials: MaterialPool::new(device.clone())?,
            frames: FrameData::new(&device)?,
            profiles: LightProfilePool::new(device.clone()),
            shadow: None,
            built: BuiltFrame::default(),
            look_state: LookState::new(&device)?,
            effects: EffectPool::default(),
            frame_scratch: FrameScratch::default(),
            graph_scratch: GraphScratch::default(),
            warned_cluster_overflow: false,
            views: Vec::new(),
            warmed_look: Warmup::default(),
            warmed_fluid: false,
            warmed_denoise: Warmup::default(),
            warmed_rays: Warmup::default(),
            warmed_traced_shading: false,
            warmed_overlays: false,
            watched: FxHashMap::default(),
            rays: match device.ray_tracing() {
                Some(_) => Some(RayScene::new(device.clone())?),
                None => None,
            },
            builtin: Builtin {
                mesh_vert: MaterialShader::builtin(shaders_3d::MESH_VERT),
                depth_vert: MaterialShader::builtin(shaders_3d::DEPTH_VERT),
                depth_frag: MaterialShader::builtin(shaders_3d::DEPTH_FRAG),
                fullscreen_vert: MaterialShader::builtin(shaders_gpu::FULLSCREEN_VERT),
                sky_frag: MaterialShader::builtin(shaders_3d::SKY_FRAG),
                clip_bounds_frag: MaterialShader::builtin(shaders_3d::CLIP_BOUNDS_FRAG),
                grid_frag: MaterialShader::builtin(shaders_3d::GRID_FRAG),
                outline_frag: MaterialShader::builtin(shaders_3d::OUTLINE_FRAG),
                clip_frag: MaterialShader::builtin(shaders_3d::CLIP_FRAG),
                outline_id_frag: MaterialShader::builtin(shaders_3d::OUTLINE_ID_FRAG),
                outline_edges: [
                    MaterialShader::builtin(shaders_3d::OUTLINE_EDGES_VERT),
                    MaterialShader::builtin(shaders_3d::OUTLINE_EDGES_FRAG),
                ],
                outline_detect_frag: MaterialShader::builtin(shaders_3d::OUTLINE_DETECT_FRAG),
                outline_resolve_frag: MaterialShader::builtin(shaders_3d::OUTLINE_RESOLVE_FRAG),
                tonemap_frag: MaterialShader::builtin(shaders_3d::TONEMAP_FRAG),
                copy_frag: MaterialShader::builtin(shaders_3d::SCENE_COPY_FRAG),
                overlay_lines: [
                    MaterialShader::builtin(shaders_3d::OVERLAY_LINES_VERT),
                    MaterialShader::builtin(shaders_3d::OVERLAY_LINES_FRAG),
                ],
                overlay_points: [
                    MaterialShader::builtin(shaders_3d::OVERLAY_POINTS_VERT),
                    MaterialShader::builtin(shaders_3d::OVERLAY_POINTS_FRAG),
                ],
                overlay_triangles: [
                    MaterialShader::builtin(shaders_3d::OVERLAY_TRIANGLES_VERT),
                    MaterialShader::builtin(shaders_3d::OVERLAY_TRIANGLES_FRAG),
                ],
                workbench_frag: MaterialShader::builtin(shaders_3d::WORKBENCH_EFFECTS_FRAG),
                editor_grid: [
                    MaterialShader::builtin(shaders_3d::EDITOR_GRID_VERT),
                    MaterialShader::builtin(shaders_3d::EDITOR_GRID_FRAG),
                ],
            },
            device,
        };
        for recipe in recipes::frame_wide(&renderer.builtin) {
            renderer.pipelines.prepare(&recipe);
        }
        renderer
            .pipelines
            .prepare_kernel(shaders_3d::LTC_TABLE_COMP);
        renderer
            .pipelines
            .prepare_kernel(shaders_3d::MESH_ATTRIBUTES_COMP);
        Ok(renderer)
    }

    pub(crate) fn precompile_for(&mut self, frame: &Frame) {
        if !frame.overlays.is_empty() {
            self.precompile_overlays();
        }
        let Some(look) = &frame.look else {
            return self.precompile_fluid(frame);
        };
        if !self.warmed_look.is_started() {
            if look.composite.draws_emission(&self.effects) {
                let emissive: Vec<ViewFormats> = self
                    .views
                    .iter()
                    .map(|view| ViewFormats {
                        emission: true,
                        ..*view
                    })
                    .collect();
                for view in emissive {
                    self.precompile_view(view);
                }
            }
            let kernels: Vec<Shader> = sky::KERNELS
                .into_iter()
                .chain(space::KERNELS)
                .chain(post::KERNELS)
                .chain(pbr::KERNELS)
                .collect();
            let recipes = self.look_recipes();
            self.warmed_look
                .start(&mut self.pipelines, &kernels, &recipes);
        }
        if let Ok(tiers) = look.tiers(self.rays.is_some()) {
            if denoises(tiers) {
                self.warmed_denoise
                    .start(&mut self.pipelines, &ray::SCREEN_KERNELS, &[]);
            }
            if tiers.traces() {
                self.warmed_rays
                    .start(&mut self.pipelines, &ray::KERNELS, &[]);
            }
            if !self.warmed_traced_shading && tiers.shadows == Mode::RayTraced {
                self.warmed_traced_shading = true;
                let traced: Vec<ViewFormats> = self
                    .views
                    .iter()
                    .map(|view| ViewFormats {
                        tracing: Tracing::RayQueries,
                        ..*view
                    })
                    .collect();
                for view in traced {
                    self.precompile_view(view);
                }
            }
        }
        self.precompile_fluid(frame);
    }

    pub(crate) fn look_ready(&self, look: &Look) -> bool {
        let Ok(tiers) = look.tiers(self.rays.is_some()) else {
            return true;
        };
        self.warmed_look.is_ready()
            && (!denoises(tiers) || self.warmed_denoise.is_ready())
            && (!tiers.traces() || self.warmed_rays.is_ready())
    }

    fn look_recipes(&self) -> Vec<Recipe> {
        let look = &self.look_state;
        let mut recipes = vec![look.sky.fog_recipe(&self.builtin)];
        recipes.extend(look.post.recipes(&self.builtin));
        recipes.extend(look.space.filter_recipes(&self.builtin));
        recipes.extend(
            self.views
                .iter()
                .map(|view| look.space.particles_recipe(*view)),
        );
        recipes.extend(
            self.views
                .iter()
                .map(|view| look.space.emitters.shade_recipe(*view)),
        );
        recipes.extend(
            self.views
                .iter()
                .map(|view| look.sky.panorama_recipe(&self.builtin, *view)),
        );
        recipes.extend(
            self.materials
                .descs()
                .filter(|desc| desc.state.pass.in_prepass() && !desc.hull)
                .map(|desc| {
                    recipes::surfaces(
                        &look.surface_vert,
                        &desc.program,
                        desc.state,
                        surface_formats(false),
                        false,
                        desc.state.pass == MaterialPass::Masked,
                    )
                }),
        );
        for view in self.views.iter().filter(|view| view.emission) {
            recipes.extend(recipes::view_wide(&self.builtin, *view));
            for desc in self.materials.descs() {
                let used = (desc.state, desc.hull, features::of(desc));
                recipes.extend(recipes::material(&self.builtin, &desc.program, used, *view));
            }
        }
        recipes
    }

    fn precompile_fluid(&mut self, frame: &Frame) {
        if !frame.fluids.is_empty() && !self.warmed_fluid {
            self.warmed_fluid = true;
            for spirv in fluid::KERNELS {
                self.pipelines.prepare_kernel(spirv);
            }
        }
    }

    pub(crate) fn create_mesh(&mut self, data: &MeshData<'_>) -> Result<MeshId> {
        self.meshes.create(data)
    }

    pub(crate) fn destroy_mesh(&mut self, id: MeshId) -> bool {
        self.forget_structures(id);
        self.meshes.destroy(id)
    }

    pub(crate) fn replace_mesh(&mut self, id: MeshId, data: &MeshData<'_>) -> Result<()> {
        self.meshes.replace(id, data)?;
        self.forget_structures(id);
        Ok(())
    }

    pub(crate) fn update_mesh(&mut self, id: MeshId, update: &MeshUpdate<'_>) -> Result<()> {
        self.meshes.update(id, update)?;
        if !update.positions.is_empty() {
            self.forget_structures(id);
        }
        Ok(())
    }

    pub(crate) fn replace_mesh_indices(
        &mut self,
        id: MeshId,
        indices: Indices<'_>,
        submeshes: &[Submesh],
    ) -> Result<()> {
        self.meshes.replace_indices(id, indices, submeshes)?;
        self.forget_structures(id);
        Ok(())
    }

    pub(crate) fn set_mesh_edges(
        &mut self,
        id: MeshId,
        submesh: u32,
        edges: &[[u32; 4]],
    ) -> Result<()> {
        self.meshes.set_edges(id, submesh, edges)
    }

    fn forget_structures(&mut self, id: MeshId) {
        if let Some(rays) = &mut self.rays {
            rays.forget(id);
        }
    }

    pub(crate) fn create_material(
        &mut self,
        desc: MaterialDesc,
        finished: u64,
        texture_slot: impl Fn(TextureId) -> Option<u32>,
    ) -> Result<MaterialId> {
        if desc
            .program
            .shaders()
            .iter()
            .any(MaterialShader::uses_storage_images)
            && self.device.storage_image_capacity() == 0
        {
            return Err(Error::unsupported(
                "material writes storage images, but the device table has none".into(),
            ));
        }
        let id = self.materials.create(desc, finished, texture_slot)?;
        if let Some(desc) = self.materials.describe(id) {
            let (template, program) = (desc.template.clone(), Arc::clone(&desc.program));
            let state = desc.state;
            if self.rays.is_some() && program.deforms {
                self.pipelines
                    .prepare_material_kernel(program.shader(Variant::Hook));
            }
            if self.warmed_overlays {
                let occluder = recipes::occluder(&self.builtin, &program, state);
                self.pipelines.prepare(&occluder);
            }
            let recipes =
                self.recipes_for(&program, &[(desc.state, desc.hull, features::of(desc))]);
            self.watch(&template, &program, &recipes);
        }
        Ok(id)
    }

    fn precompile_view(&mut self, view: ViewFormats) {
        if self.views.contains(&view) {
            return;
        }
        self.views.push(view);
        for recipe in recipes::view_wide(&self.builtin, view) {
            self.pipelines.prepare(&recipe);
        }
        let used: Vec<_> = self
            .materials
            .descs()
            .map(|desc| {
                let uses = [(desc.state, desc.hull, features::of(desc))];
                (desc.template.clone(), Arc::clone(&desc.program), uses)
            })
            .collect();
        for (template, program, uses) in used {
            let recipes = self.recipes_for(&program, &uses);
            self.watch(&template, &program, &recipes);
        }
    }

    pub(crate) fn create_effect(&mut self, effect: &Effect) -> Result<EffectId> {
        let id = self
            .effects
            .create(effect, &mut self.pipelines, &self.builtin)?;
        self.prepare_effect_support();
        Ok(id)
    }

    pub(crate) fn reload_effect(&mut self, id: EffectId, effect: &Effect) -> Result<()> {
        self.effects
            .replace(id, effect, &mut self.pipelines, &self.builtin)?;
        self.prepare_effect_support();
        Ok(())
    }

    fn prepare_effect_support(&mut self) {
        for spirv in EFFECT_KERNELS {
            self.pipelines.prepare_kernel(spirv);
        }
    }

    pub(crate) fn destroy_material(&mut self, id: MaterialId, after: u64) -> bool {
        self.materials.destroy(id, after)
    }

    pub(crate) fn set_material_values(
        &mut self,
        id: MaterialId,
        values: &[(&str, Value)],
        after: u64,
        texture_slot: impl Fn(TextureId) -> Option<u32>,
    ) -> Result<bool> {
        self.materials.set_values(id, values, after, texture_slot)
    }

    pub(crate) fn prepare_materials(
        &mut self,
        frame: &Frame,
        after: u64,
        texture_slot: impl Fn(TextureId) -> Option<u32>,
    ) -> Result<()> {
        for reload in self.materials.reloads() {
            let recipes = self.recipes_for(&reload.program, &reload.uses);
            self.watch(&reload.template, &reload.program, &recipes);
            if recipes.iter().all(|recipe| self.pipelines.ready(recipe)) {
                self.materials.adopt(&reload, after, &texture_slot)?;
                let live: FxHashSet<u64> = self
                    .materials
                    .descs()
                    .map(|desc| desc.program.generation)
                    .collect();
                self.watched
                    .retain(|generation, _| live.contains(generation));
            }
        }
        self.materials.resolve(frame, after, texture_slot)?;
        self.warm_variants();
        for desc in self.materials.descs() {
            if desc.program.wants_optimizing() {
                desc.template.optimize(&desc.program);
            }
        }
        Ok(())
    }

    fn warm_variants(&mut self) {
        if self.materials.warming().is_empty() {
            return;
        }
        let mut settled = Vec::new();
        for warming in self.materials.warming() {
            let mut ready = true;
            for &view in &self.views {
                for recipe in
                    recipes::material(&self.builtin, &warming.program, warming.used(), view)
                {
                    self.pipelines.prepare(&recipe);
                    ready &= self.pipelines.ready(&recipe);
                }
            }
            if ready {
                settled.push(warming.record);
            }
        }
        for record in settled {
            self.materials.settle(record);
        }
    }

    fn recipes_for(
        &self,
        program: &Program,
        uses: &[(MaterialState, bool, Routing)],
    ) -> Vec<Recipe> {
        let mut recipes = Vec::new();
        for &view in &self.views {
            for &used in uses {
                recipes.extend(recipes::material(&self.builtin, program, used, view));
            }
        }
        recipes
    }

    fn watch(&mut self, template: &MaterialTemplate, program: &Arc<Program>, recipes: &[Recipe]) {
        let watched = self.watched.entry(program.generation).or_default();
        let fresh: Vec<&Recipe> = recipes
            .iter()
            .filter(|recipe| watched.insert(recipe.key))
            .collect();
        if fresh.is_empty() {
            return;
        }
        let remaining = Arc::new(AtomicUsize::new(fresh.len()));
        for recipe in fresh {
            let remaining = Arc::clone(&remaining);
            let template = template.clone();
            let program = Arc::clone(program);
            self.pipelines.when_ready(recipe, move || {
                if remaining.fetch_sub(1, Ordering::AcqRel) == 1 {
                    template.mark_ready(&program);
                }
            });
        }
    }

    pub(crate) fn warn_cluster_overflow(&mut self, local: u32, stride: u32) {
        if local <= stride || self.warned_cluster_overflow {
            return;
        }
        self.warned_cluster_overflow = true;
        tracing::warn!(
            "scene has {local} local lights but a cluster holds {stride}; excess lights \
             are dropped per cluster"
        );
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.meshes.has_pending()
            || self.overlays.has_pending()
            || self.materials.has_pending()
            || self.profiles.has_pending()
    }

    fn precompile_overlays(&mut self) {
        if self.warmed_overlays {
            return;
        }
        self.warmed_overlays = true;
        for recipe in recipes::overlays(&self.builtin) {
            self.pipelines.prepare(&recipe);
        }
        let occluders: Vec<Recipe> = self
            .materials
            .descs()
            .map(|desc| recipes::occluder(&self.builtin, &desc.program, desc.state))
            .collect();
        for recipe in occluders {
            self.pipelines.prepare(&recipe);
        }
    }

    pub(crate) fn flush(&mut self, ctx: &mut GpuFrameCtx<'_>) -> Result<()> {
        self.meshes.flush(ctx, &mut self.pipelines)?;
        self.overlays.flush(ctx)?;
        self.profiles.flush(ctx)?;
        self.materials.flush(ctx)
    }

    pub(crate) fn create_viewport(
        &mut self,
        bind: &mut BindPoint<'_>,
        textures: &mut rikka_gpu::texture::TexturePool,
        config: ViewportConfig,
        reclaimer: Reclaimer,
    ) -> Result<Viewport> {
        let samples = self.device.sample_count(config.wanted_samples());
        let display = vk::Extent2D {
            width: config.width,
            height: config.height,
        };
        let temporal = match config.antialiasing.is_temporal() {
            true => Some(TemporalState::new(
                &self.device,
                config.antialiasing,
                display,
            )?),
            false => None,
        };
        let capacity = temporal
            .as_ref()
            .map_or(display, TemporalState::capacity_extent);
        let targets = Targets::create(bind, &config, samples, capacity)?;
        let output = textures.create_target(bind, config.width, config.height)?;
        let viewport = Viewport {
            config,
            samples,
            targets: Some(targets),
            output,
            drawn: false,
            look_pending: false,
            graph: crate::graph::ViewGraph::default(),
            history: crate::history::ViewHistory::default(),
            temporal,
            reclaimer,
        };
        self.precompile_view(viewport.formats());
        if viewport.temporal.is_some() {
            for spirv in temporal::KERNELS {
                self.pipelines.prepare_kernel(spirv);
            }
        }
        Ok(viewport)
    }

    pub(crate) fn ensure_shadow_atlas(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        planned: shadow::Planned,
        tinted: bool,
    ) -> Result<()> {
        shadow::store::ensure(&mut self.shadow, &self.device, ctx, planned, tinted)
    }

    pub(crate) fn device(&self) -> &Arc<RenderDevice> {
        &self.device
    }
}

impl std::fmt::Debug for Renderer3dState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Renderer3dState").finish_non_exhaustive()
    }
}

fn denoises(tiers: Tiers) -> bool {
    tiers.traces() || tiers.illumination != Mode::Off
}
