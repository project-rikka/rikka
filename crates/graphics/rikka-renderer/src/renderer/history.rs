use std::time::Instant;

use ash::vk;
use rikka_colorimetry::BlendSpace;
use rikka_types::{Rect, Transform};
use rustc_hash::FxHashMap;

use crate::fx::{FX_FORMAT, HDR_FX_FORMAT};
use crate::renderer::canvas::Plan;
use crate::scene::{DamageTracker, SceneId, Viewer};
use rikka_gpu::core::image::GpuImage;
use rikka_gpu::core::runtime::{GpuRuntime, Recycler, UNUSED_FOR};
use rikka_gpu::core::table_image::TableImage;
use rikka_gpu::error::Result;
use rikka_gpu::identity::{BufferId, Identities, TargetId};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DamageSource {
    #[default]
    SinceLastFrame,
    CompleteForBuffer,
}

struct SceneImage {
    table: TableImage,
    resting: Option<Instant>,
}

impl SceneImage {
    fn has_aged(&self, now: Instant) -> bool {
        self.resting
            .is_some_and(|since| now.duration_since(since) >= UNUSED_FOR)
    }
}

impl SceneImage {
    fn create(gpu: &mut GpuRuntime, extent: vk::Extent2D, format: vk::Format) -> Result<Self> {
        let image = GpuImage::new(
            gpu.device().clone(),
            "rikka.scene",
            extent,
            format,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        )?;
        let bind = gpu.bind_point();
        Ok(Self {
            table: TableImage::register(image, bind.resources, bind.samplers.linear_clamp())?,
            resting: None,
        })
    }

    fn retire(self, gpu: &mut GpuRuntime) {
        self.table.retire(&mut gpu.recycler());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Viewing {
    pub(crate) scene: SceneId,
    pub(crate) viewer: Viewer,
}

pub(crate) struct TargetState {
    scene_color: Option<SceneImage>,
    extent: vk::Extent2D,
    transform: Transform,
    space: BlendSpace,
    viewing: Viewing,
    buffers: FxHashMap<BufferId, DamageTracker>,
    scene_missed: DamageTracker,
    latest_dirty: bool,
}

impl TargetState {
    fn new(plan: &Plan, viewing: Viewing) -> Self {
        Self {
            scene_color: None,
            extent: plan.scene_extent(),
            transform: plan.transform,
            space: plan.space,
            viewing,
            buffers: FxHashMap::default(),
            scene_missed: DamageTracker::default(),
            latest_dirty: false,
        }
    }

    pub(crate) fn holds_scene(&self) -> bool {
        self.scene_color
            .as_ref()
            .is_some_and(|scene| scene.resting.is_none())
    }

    pub(crate) fn scene_color_mut(&mut self) -> Option<&mut TableImage> {
        self.scene_color.as_mut().map(|scene| &mut scene.table)
    }

    fn up_to_date(&self, buffer: BufferId) -> bool {
        self.buffers
            .get(&buffer)
            .is_some_and(DamageTracker::is_empty)
    }

    fn merge_missed(&self, current: &[Rect], buffer: BufferId, merger: &mut DamageTracker) {
        merger.add_all(current.iter().copied());
        match self.buffers.get(&buffer) {
            Some(missed) => merger.add_tracked(missed),
            None => merger.mark_full(),
        }
    }

    fn merge_scene_missed(&self, current: &[Rect], merger: &mut DamageTracker) {
        merger.add_all(current.iter().copied());
        merger.add_tracked(&self.scene_missed);
    }

    fn record(&mut self, buffer: BufferId, entry: &[Rect], composed: bool) {
        for (known, missed) in &mut self.buffers {
            match *known == buffer {
                true => missed.reset(),
                false => missed.add_all(entry.iter().copied()),
            }
        }
        self.buffers.entry(buffer).or_default();
        if composed {
            self.scene_missed.reset();
        }
        self.latest_dirty = false;
    }

    fn missed(&mut self, entry: &[Rect]) {
        if entry.is_empty() {
            return;
        }
        for missed in self.buffers.values_mut() {
            missed.add_all(entry.iter().copied());
        }
        self.scene_missed.add_all(entry.iter().copied());
        self.latest_dirty = true;
    }
}

#[derive(Default)]
pub(crate) struct Targets {
    states: FxHashMap<TargetId, TargetState>,
    merger: DamageTracker,
    ids: Identities,
}

impl Targets {
    pub(crate) fn new_target(&mut self) -> TargetId {
        self.ids.target()
    }

    pub(crate) fn new_external(&mut self) -> BufferId {
        self.ids.external()
    }

    pub(crate) fn get(&self, id: TargetId) -> Option<&TargetState> {
        self.states.get(&id)
    }

    pub(crate) fn state(&mut self, id: TargetId) -> &mut TargetState {
        self.states
            .get_mut(&id)
            .expect("target state exists before drawing")
    }

    pub(crate) fn damage_for_buffer(
        &mut self,
        id: TargetId,
        current: &[Rect],
        buffer: BufferId,
        viewport: Rect,
        into: &mut Vec<Rect>,
    ) {
        let Self { states, merger, .. } = self;
        states
            .get(&id)
            .expect("target state exists before drawing")
            .merge_missed(current, buffer, merger);
        merger.take_into(viewport, into);
    }

    pub(crate) fn damage_for_scene(
        &mut self,
        id: TargetId,
        current: &[Rect],
        viewport: Rect,
        into: &mut Vec<Rect>,
    ) {
        let Self { states, merger, .. } = self;
        states
            .get(&id)
            .expect("target state exists before drawing")
            .merge_scene_missed(current, merger);
        merger.take_into(viewport, into);
    }

    pub(crate) fn record(
        &mut self,
        id: TargetId,
        buffer: BufferId,
        entry: &[Rect],
        composed: bool,
    ) {
        let state = self.state(id);
        state.record(buffer, entry, composed);
        let viewing = state.viewing;
        for (other, state) in &mut self.states {
            if *other != id && state.viewing == viewing {
                state.missed(entry);
            }
        }
    }

    pub(crate) fn up_to_date(&self, id: TargetId, buffer: BufferId) -> bool {
        self.states
            .get(&id)
            .is_some_and(|state| state.up_to_date(buffer))
    }

    pub(crate) fn is_behind(&self, id: TargetId) -> bool {
        self.states.get(&id).is_some_and(|state| state.latest_dirty)
    }

    pub(crate) fn views(&self, id: TargetId, viewing: Viewing) -> bool {
        self.states
            .get(&id)
            .is_some_and(|state| state.viewing == viewing)
    }

    pub(crate) fn needs_full(&self, plan: &Plan) -> bool {
        match self.states.get(&plan.target) {
            Some(state) => {
                state.extent != plan.scene_extent()
                    || state.transform != plan.transform
                    || state.space != plan.space
            }
            None => true,
        }
    }

    pub(crate) fn forget_buffer(&mut self, buffer: BufferId) {
        for state in self.states.values_mut() {
            state.buffers.remove(&buffer);
        }
    }

    pub(crate) fn retire(&mut self, gpu: &mut GpuRuntime, id: TargetId) {
        if let Some(mut state) = self.states.remove(&id)
            && let Some(scene) = state.scene_color.take()
        {
            scene.retire(gpu);
        }
    }

    pub(crate) fn prepare(&mut self, gpu: &mut GpuRuntime, plan: &Plan, viewing: Viewing) -> bool {
        let id = plan.target;
        if !self.needs_full(plan) {
            let state = self.state(id);
            if state.viewing == viewing {
                return false;
            }
            state.viewing = viewing;
            state.buffers.clear();
            state.scene_missed.reset();
            state.latest_dirty = false;
            return true;
        }
        self.retire(gpu, id);
        self.states.insert(id, TargetState::new(plan, viewing));
        true
    }

    pub(crate) fn ensure_scene_color(
        &mut self,
        gpu: &mut GpuRuntime,
        id: TargetId,
        format: vk::Format,
    ) -> Result<bool> {
        let state = self.state(id);
        if let Some(scene) = state
            .scene_color
            .take_if(|scene| scene.table.image.format() != format)
        {
            scene.retire(gpu);
        }
        let state = self.state(id);
        if let Some(scene) = &mut state.scene_color {
            let stale = scene.resting.is_some();
            if stale {
                scene.resting = None;
                scene.table.image.discard();
            }
            return Ok(stale);
        }
        let extent = state.extent;
        let scene = SceneImage::create(gpu, extent, format)?;
        self.state(id).scene_color = Some(scene);
        Ok(true)
    }

    pub(crate) fn scene_format(space: &BlendSpace) -> vk::Format {
        match space {
            BlendSpace::Sdr => FX_FORMAT,
            BlendSpace::Hdr(_) => HDR_FX_FORMAT,
        }
    }

    pub(crate) fn rest_scene_color(&mut self, gpu: &mut GpuRuntime, id: TargetId) {
        let state = self.state(id);
        let Some(scene) = &mut state.scene_color else {
            return;
        };
        let now = Instant::now();
        scene.resting.get_or_insert(now);
        if scene.has_aged(now)
            && let Some(scene) = state.scene_color.take()
        {
            scene.retire(gpu);
        }
    }

    pub(crate) fn trim(&mut self, recycler: &mut Recycler<'_>, now: Instant) {
        for state in self.states.values_mut() {
            if state
                .scene_color
                .as_ref()
                .is_some_and(|scene| scene.has_aged(now))
                && let Some(scene) = state.scene_color.take()
            {
                scene.table.retire(recycler);
            }
        }
    }
}
