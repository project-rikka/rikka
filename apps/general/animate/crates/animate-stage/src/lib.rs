mod actor;
mod booth;
mod capture;
mod cloth;
mod dressing;
mod error;
mod floor;
mod fluid;
mod holding;
mod lens;
mod locator;
mod marks;
mod pacer;
mod pending;
mod physics;
mod pour;
mod props;
mod recorder;
mod render;
mod scene;
mod unkeyed;
mod wind;

use std::sync::Arc;
use std::time::Instant;

use animate_body::MorphWeights;
use animate_cast::{
    Figure, FigureId, LiquidCache, PropId, SharedClock, Shot, TakePlayback, frame_of, with_clock,
};
use animate_look::Look;
use animate_motion::CameraMotion;
use rikka_model::{Bounds, MaterialId};
use rikka_model_gpu::Prepared;
use rikka_physics::Progress;
use rikka_physics_tailor::Quality;
use rikka_renderer::Uploads;
use rikka_sync::Mutex;
use rikka_types::PixelSize;
use rikka_ui::Watchers;
use rikka_viewport::SceneCtx;
use rustc_hash::{FxHashMap, FxHashSet};

pub use actor::Projection;
pub use booth::{Sitting, Studio, rest, still_viewport};
pub use capture::{Capture, CaptureResult, CaptureView, Picture};
pub use cloth::PaintSurface;
pub use dressing::{Dressing, is_color_lookup};
pub use error::StageError;
pub use fluid::{Aiming, Joint, Liquids, Placed, Placing};
use holding::Pinches;
pub use holding::{ClothSnapshot, Spot};
pub use lens::{Lens, project};
pub use marks::{EmitterMark, EmitterMarks};
pub use pending::{Local, Nudge, Pending, PendingPose, Posture, Space, nudged};
use pour::CastMember;
pub use pour::{bake, frame_at_step};
pub use props::PropFrame;
pub use recorder::Recorder;
pub use render::Render;
pub use rikka_physics::ProgressCallback as Notify;
use scene::Scene;
pub use unkeyed::{Drafts, Unkeyed};
pub use wind::{Blowing, Vane};

pub type SharedStage = Arc<Mutex<Stage>>;

pub type SharedRender = Arc<Mutex<Render>>;

#[derive(Debug, Default)]
struct Outputs {
    revision: u64,
    projection: Option<Projection>,
    aiming: Option<Aiming>,
    emitters: Option<Arc<EmitterMarks>>,
    vane: Option<Vane>,
    paint_surface: Option<Arc<PaintSurface>>,
    cloth_snapshot: Option<Arc<ClothSnapshot>>,
    graspable: bool,
    restless: bool,
    posed_at: Option<f32>,
    size: PixelSize,
    due: Option<Instant>,
    bounds: FxHashMap<FigureId, Bounds>,
    prop_bounds: Vec<Bounds>,
    prop_frame: Option<PropFrame>,
    progress: Option<Progress>,
}

pub(crate) struct Input {
    scene: Scene,
    revision: u64,
    time: f32,
    running: bool,
    length: f32,
    prepared: Vec<(FigureId, Prepared)>,
    prop_prepared: Vec<(PropId, Prepared)>,
    captures: Vec<Capture>,
    resourced: Option<Option<u32>>,
    wants_snapshot: bool,
}

pub(crate) struct Published {
    outputs: Outputs,
    errors: Vec<StageError>,
    drew: bool,
    uploads: Uploads,
}

pub struct Stage {
    clock: SharedClock,
    scene: Scene,
    revision: u64,
    seated_at: Option<f32>,
    prepared: FxHashMap<FigureId, Prepared>,
    prop_prepared: FxHashMap<PropId, Prepared>,
    supplied: FxHashSet<PropId>,
    captures: Vec<Capture>,
    resourced: Option<Option<u32>>,
    roused: bool,
    wants_snapshot: bool,
    outputs: Outputs,
    error: Option<StageError>,
    watchers: Watchers,
    uploads: Option<Uploads>,
}

impl Stage {
    pub fn new(clock: SharedClock, physics: bool, quality: Quality) -> Self {
        Self {
            clock,
            scene: Scene::new(physics, quality),
            revision: 1,
            seated_at: None,
            prepared: FxHashMap::default(),
            prop_prepared: FxHashMap::default(),
            supplied: FxHashSet::default(),
            captures: Vec::new(),
            resourced: Some(Some(0)),
            roused: false,
            wants_snapshot: false,
            outputs: Outputs::default(),
            error: None,
            watchers: Watchers::new(),
            uploads: None,
        }
    }

    fn change(&mut self) -> &mut Scene {
        self.revision += 1;
        &mut self.scene
    }

    fn resource(&mut self, since: Option<u32>) {
        self.revision += 1;
        self.resourced = Some(match (self.resourced, since) {
            (Some(Some(known)), Some(since)) => Some(known.min(since)),
            (Some(Some(known)), None) => Some(known),
            (Some(None) | None, since) => since,
        });
    }

    pub fn add_figure(&mut self, figure: &Figure, prepared: Prepared) {
        self.change().members.push(CastMember {
            id: figure.id,
            name: figure.name.clone(),
            pinches: Pinches::new(figure.body.as_deref(), &figure.model),
            model: Arc::clone(&figure.model),
            anatomy: Arc::clone(&figure.anatomy),
            skeleton: Arc::clone(&figure.skeleton),
            body: figure.body.clone(),
            faces: Arc::clone(&figure.faces),
            head: figure.head,
            playback: None,
        });
        self.prepared.insert(figure.id, prepared);
        self.resource(None);
    }

    pub fn dismiss(&mut self, id: FigureId) {
        self.prepared.remove(&id);
        let scene = self.change();
        scene.members.retain(|member| member.id != id);
        if scene
            .pending
            .as_ref()
            .is_some_and(|pending| pending.figure == id)
        {
            scene.pending = None;
        }
        if scene.focus == Some(id) {
            scene.focus = None;
        }
        scene.figure_cloth.remove(&id);
        if scene.paint_figure == Some(id) {
            scene.paint_figure = None;
        }
        if self
            .outputs
            .projection
            .as_ref()
            .is_some_and(|projection| projection.figure == id)
        {
            self.outputs.projection = None;
        }
        self.outputs.paint_surface = None;
        self.resource(None);
    }

    pub fn pose(
        &mut self,
        id: FigureId,
        playback: Option<Arc<TakePlayback>>,
        changed: Option<u32>,
    ) {
        if let Some(member) = self
            .change()
            .members
            .iter_mut()
            .find(|member| member.id == id)
        {
            member.playback = playback;
        }
        self.resource(changed.map(pour::step_of_frame));
    }

    pub fn pause_liquids(&mut self, paused: bool) {
        self.change().liquid_paused = paused;
    }

    pub fn recompute_liquids(&mut self) {
        self.change().recomputes += 1;
    }

    pub fn liquid_cache(&self) -> LiquidCache {
        self.outputs
            .progress
            .as_ref()
            .map_or_else(LiquidCache::default, pour::cache_of)
    }

    pub fn focus(&mut self, figure: Option<FigureId>) {
        if self.scene.focus != figure {
            self.change().focus = figure;
            self.outputs.projection = None;
        }
    }

    pub fn projection(&self) -> Option<&Projection> {
        self.outputs.projection.as_ref()
    }

    pub fn emitter_marks(&self) -> Option<&EmitterMarks> {
        self.outputs.emitters.as_deref()
    }

    pub fn watchers(&self) -> &Watchers {
        &self.watchers
    }

    pub fn pending_at(&self, figure: FigureId, frame: u32) -> Option<&Pending> {
        self.scene
            .pending
            .as_ref()
            .filter(|pending| pending.figure == figure && pending.frame == frame)
    }

    pub fn take_pending(&mut self, figure: FigureId, frame: u32) -> Option<Pending> {
        let pending = self.change().pending.take()?;
        (pending.figure == figure && pending.frame == frame).then_some(pending)
    }

    fn pending_for(&mut self, figure: FigureId, frame: u32) -> &mut Pending {
        match &mut self.change().pending {
            Some(pending) if pending.figure == figure && pending.frame == frame => pending,
            slot => slot.insert(Pending::new(figure, frame)),
        }
    }

    pub fn hold(&mut self, figure: FigureId, frame: u32, bone: usize, local: Local) {
        let pending = self.pending_for(figure, frame);
        match &mut pending.pose {
            Some(PendingPose::Bones(bones)) => {
                bones.insert(bone, local);
            }
            slot => *slot = Some(PendingPose::Bones(FxHashMap::from_iter([(bone, local)]))),
        }
    }

    pub fn preview(&mut self, figure: FigureId, frame: u32, posture: Posture) {
        self.pending_for(figure, frame).pose = Some(PendingPose::Human(Box::new(posture)));
    }

    pub fn hold_morphs(&mut self, figure: FigureId, frame: u32, morphs: MorphWeights) {
        let pending = self.pending_for(figure, frame);
        pending.morphs = morphs;
        if pending.is_empty() {
            self.scene.pending = None;
        }
    }

    pub fn discard_pending_pose(&mut self, figure: FigureId, frame: u32) {
        let pending = self.pending_for(figure, frame);
        pending.pose = None;
        if pending.is_empty() {
            self.scene.pending = None;
        }
    }

    pub fn release(&mut self) {
        if self.scene.pending.is_some() {
            self.change().pending = None;
        }
    }

    pub fn drafts(&self) -> &Drafts {
        &self.scene.drafts
    }

    pub fn drafts_mut(&mut self) -> &mut Drafts {
        &mut self.change().drafts
    }

    pub fn set_physics(&mut self, physics: bool) {
        if self.scene.physics != physics {
            self.change().physics = physics;
            self.resource(None);
        }
    }

    pub fn reset_physics(&mut self) {
        self.change().restarts += 1;
        self.resource(Some(0));
    }

    pub fn set_cloth_quality(&mut self, quality: Quality) {
        if self.scene.quality != quality {
            self.change().quality = quality;
            self.resource(None);
        }
    }

    pub fn is_restless(&self) -> bool {
        self.outputs.restless || !self.captures.is_empty()
    }

    pub fn next_draw(&self, now: Instant, size: PixelSize) -> Option<Instant> {
        let (time, _) = self.clock_at(now);
        let stale = self.outputs.revision != self.revision
            || self.roused
            || self.is_restless()
            || self.outputs.size != size
            || self.outputs.posed_at != Some(time);
        stale.then(|| self.outputs.due.map_or(now, |due| due.max(now)))
    }

    pub fn rouse(&mut self) {
        self.roused = true;
    }

    pub fn set_camera(&mut self, camera: Option<Arc<CameraMotion>>) {
        self.change().camera = camera;
        self.seated_at = None;
    }

    pub fn follow_camera(&mut self, follow: bool) {
        if self.scene.follows_camera == follow {
            return;
        }
        self.change().follows_camera = follow;
        self.seated_at = None;
    }

    pub fn turn(&mut self, x: f32, y: f32) {
        self.view_mut().turn(x, y);
    }

    pub fn pan(&mut self, x: f32, y: f32, height: f32) {
        self.view_mut().pan(x, y, height);
    }

    pub fn dolly(&mut self, notches: f32) {
        self.view_mut().dolly(notches);
    }

    pub fn reframe(&mut self, change: impl FnOnce(&mut Shot)) {
        change(self.view_mut());
    }

    pub fn current_shot(&mut self) -> Shot {
        let (time, running) = self.input_time();
        self.seat(time, running);
        self.scene.shot(time, running)
    }

    pub fn rest(&mut self) {
        *self.view_mut() = Shot::default();
    }

    pub fn frame_all(&mut self) {
        let Some(bounds) = self
            .outputs
            .bounds
            .values()
            .chain(&self.outputs.prop_bounds)
            .copied()
            .reduce(|one, two| Bounds {
                min: one.min.min(two.min),
                max: one.max.max(two.max),
            })
        else {
            self.rest();
            return;
        };
        *self.view_mut() = Shot::framing(bounds);
    }

    pub fn bounds_of(&self, figure: FigureId) -> Option<Bounds> {
        self.outputs.bounds.get(&figure).copied()
    }

    pub fn take_error(&mut self) -> Option<StageError> {
        self.error.take()
    }

    pub fn touch(&mut self) {
        self.change();
    }

    pub fn set_look(&mut self, look: Arc<Look>) {
        if !Arc::ptr_eq(&self.scene.look, &look) {
            self.change().look = look;
        }
    }

    pub fn highlight(&mut self, figure: FigureId, materials: &[u32]) {
        let wanted = (!materials.is_empty()).then(|| {
            let ids: Arc<[MaterialId]> = materials.iter().copied().map(MaterialId).collect();
            (figure, ids)
        });
        if self.scene.highlight != wanted {
            self.change().highlight = wanted;
        }
    }

    pub fn capture(&mut self, capture: Capture) {
        self.captures.push(capture);
    }

    fn view_mut(&mut self) -> &mut Shot {
        let (time, running) = self.input_time();
        self.seat(time, running);
        &mut self.change().free
    }

    fn input_time(&self) -> (f32, bool) {
        self.clock_at(Instant::now())
    }

    fn clock_at(&self, now: Instant) -> (f32, bool) {
        with_clock(&self.clock, |clock| (clock.time(now), clock.is_running()))
    }

    fn seat(&mut self, time: f32, running: bool) {
        if running {
            self.seated_at = None;
            return;
        }
        if self.seated_at == Some(time) {
            return;
        }
        if let Some(pose) = self
            .scene
            .camera
            .as_ref()
            .filter(|_| self.scene.follows_camera)
            .and_then(|camera| camera.pose_at(time))
        {
            self.change().free = Shot::from(pose);
            self.seated_at = Some(time);
        }
    }

    fn take_input(&mut self, now: Instant) -> Input {
        let (time, running) = self.clock_at(now);
        self.seat(time, running);
        self.roused = false;
        let frame = frame_of(time);
        if self
            .scene
            .pending
            .as_ref()
            .is_some_and(|pending| running || pending.frame != frame)
        {
            self.change().pending = None;
        }
        if self.scene.drafts.is_stale(running, frame) {
            self.change().drafts.discard_stale(running, frame);
        }
        Input {
            scene: self.scene.clone(),
            revision: self.revision,
            time,
            running,
            length: with_clock(&self.clock, |clock| clock.length()),
            prepared: self.prepared.drain().collect(),
            prop_prepared: self.prop_prepared.drain().collect(),
            captures: std::mem::take(&mut self.captures),
            resourced: self.resourced.take(),
            wants_snapshot: std::mem::take(&mut self.wants_snapshot),
        }
    }

    pub fn uploads(&self) -> Option<Uploads> {
        self.uploads.clone()
    }

    fn publish(&mut self, published: Published) {
        let Published {
            outputs,
            errors,
            drew,
            uploads,
        } = published;
        let changed = self.outputs.revision != outputs.revision
            || self.outputs.restless != outputs.restless
            || !errors.is_empty();
        self.outputs = outputs;
        self.uploads.get_or_insert(uploads);
        if let Some(error) = errors.into_iter().last() {
            self.error = Some(error);
        }
        if drew || changed {
            self.watchers.repaint();
        }
    }

    fn replica_input(&self) -> Input {
        Input {
            scene: self.scene.clone(),
            revision: self.revision,
            time: 0.0,
            running: false,
            length: with_clock(&self.clock, |clock| clock.length()),
            prepared: Vec::new(),
            prop_prepared: Vec::new(),
            captures: Vec::new(),
            resourced: Some(Some(0)),
            wants_snapshot: false,
        }
    }
}

pub fn draw(
    stage: &SharedStage,
    render: &SharedRender,
    ctx: SceneCtx<'_>,
) -> Option<rikka_renderer_3d::Frame> {
    let now = Instant::now();
    let mut render = render.lock();
    let input = stage.lock().take_input(now);
    let (frame, published) = render.draw(input, ctx, now);
    stage.lock().publish(published);
    frame
}

pub struct Replica {
    input: Input,
    render: Render,
}

pub fn replica(stage: &SharedStage, render: &SharedRender) -> Replica {
    let render = render.lock();
    let stage = stage.lock();
    Replica {
        input: stage.replica_input(),
        render: render.replica(),
    }
}

impl std::fmt::Debug for Replica {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Replica")
            .field("members", &self.input.scene.members.len())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stage")
            .field("members", &self.scene.members.len())
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}
