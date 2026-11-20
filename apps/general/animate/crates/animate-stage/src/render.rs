use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use animate_cast::FigureId;
use animate_fluid::Wetness;
use animate_wind::WindMotion;
use rikka_model_gpu::{GpuModel, Prepared, UploadOptions};
use rikka_physics::{Bake, FigureKey, Playback, ProgressCallback, SceneFrame, SceneSource};
use rikka_renderer::Renderer;
use rikka_renderer_3d::{Camera, DEFAULT_LIGHT_CHANNELS, Frame, Outline};
use rikka_types::PixelSize;
use rikka_viewport::SceneCtx;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::actor::Actor;
use crate::capture::PendingCapture;
use crate::dressing::{Dressing, Moment, Player};
use crate::pacer::Pacer;
use crate::physics::{Simulation, StepInput};
use crate::pour::{CastMember, LiquidScene, step_of};
use crate::scene::Scene;
use crate::{Drafts, EmitterMarks, Input, Outputs, Published, StageError};

const LONGEST_STEP: f32 = 0.25;
const FIGURE_CHANNELS: usize = 31;

fn figure_channel(at: usize) -> u32 {
    1 << (1 + at % FIGURE_CHANNELS)
}

#[derive(Debug, Default)]
pub(super) struct Seen {
    revision: u64,
    wind: Arc<WindMotion>,
    drafts: Drafts,
    physics: Option<bool>,
    restarts: u64,
    recomputes: u64,
    paused: bool,
}

pub struct Render {
    pub(super) actors: Vec<Actor>,
    pub(super) gpus: FxHashMap<FigureId, GpuModel>,
    pub(super) prepared: FxHashMap<FigureId, Prepared>,
    pub(super) simulation: Simulation,
    pub(super) dressing: Dressing,
    pub(super) liquid: Playback,
    pub(super) liquid_scene: Arc<LiquidScene>,
    pub(super) pacer: Pacer,
    pub(super) captures: VecDeque<PendingCapture>,
    pub(super) posed_at: Option<f32>,
    pub(super) drawn_at: Option<f32>,
    pub(super) size: PixelSize,
    pub(super) dirty: bool,
    pub(super) seen: Seen,
    pub(super) errors: Vec<StageError>,
    pub(super) notify: Option<ProgressCallback>,
    pub(super) outputs: Outputs,
    pub(super) unloadable: FxHashSet<FigureId>,
}

impl Render {
    pub fn new(physics: bool, dressing: Dressing) -> Self {
        Self {
            actors: Vec::new(),
            gpus: FxHashMap::default(),
            prepared: FxHashMap::default(),
            simulation: Simulation::new(physics),
            dressing,
            liquid: Playback::Off,
            liquid_scene: Arc::default(),
            pacer: Pacer::default(),
            captures: VecDeque::new(),
            posed_at: None,
            drawn_at: None,
            size: PixelSize::ZERO,
            dirty: true,
            seen: Seen::default(),
            errors: Vec::new(),
            notify: None,
            outputs: Outputs::default(),
            unloadable: FxHashSet::default(),
        }
    }

    pub fn attach_bake(&mut self, bake: Bake) {
        self.liquid = Playback::baked(bake);
    }

    pub fn notify_with(&mut self, notify: ProgressCallback) {
        self.dressing.notify_with(Arc::clone(&notify));
        self.notify = Some(notify);
    }

    pub(super) fn replica(&self) -> Render {
        let mut replica = Render::new(self.simulation.is_enabled(), self.dressing.replica());
        replica.simulation = self.simulation.replicate();
        replica.liquid = Playback::simulated();
        replica
    }

    pub(super) fn fail(&mut self, error: StageError) {
        self.errors.push(error);
        if let Some(notify) = &self.notify {
            notify();
        }
    }

    fn scene_source(&self) -> Arc<dyn SceneSource> {
        Arc::clone(&self.liquid_scene) as Arc<dyn SceneSource>
    }

    pub(super) fn sync(&mut self, input: &mut Input) {
        self.prepared.extend(input.prepared.drain(..));
        self.captures
            .extend(input.captures.drain(..).map(PendingCapture::new));
        let scene = &input.scene;
        if self.seen.revision == input.revision {
            return;
        }
        self.seen.revision = input.revision;
        self.dirty = true;
        let cast = self.actors.len();
        self.actors
            .retain(|actor| scene.members.iter().any(|member| member.id == actor.id));
        let gone: Vec<FigureId> = self
            .gpus
            .keys()
            .copied()
            .filter(|id| !scene.members.iter().any(|member| member.id == *id))
            .collect();
        for id in gone {
            self.gpus.remove(&id);
            self.dressing.forget(id);
            self.simulation.forget(id);
        }
        self.prepared
            .retain(|id, _| scene.members.iter().any(|member| member.id == *id));
        if self.actors.len() != cast {
            self.simulation.recast();
            self.posed_at = None;
        }
        for actor in &mut self.actors {
            let Some(member) = scene.members.iter().find(|member| member.id == actor.id) else {
                continue;
            };
            let replayed = match (&actor.playback, &member.playback) {
                (Some(one), Some(two)) => !Arc::ptr_eq(one, two),
                (one, two) => one.is_some() != two.is_some(),
            };
            if replayed {
                actor.playback = member.playback.clone();
                actor.rested = false;
                self.posed_at = None;
            }
            if actor.hold(scene.pending.as_ref()) {
                self.posed_at = None;
            }
        }
        let mut resource = input.resourced;
        if self.seen.physics != Some(scene.physics) {
            self.seen.physics = Some(scene.physics);
            if self.simulation.set_mode(scene.physics) {
                self.posed_at = None;
                resource.get_or_insert(None);
            }
        }
        if self.simulation.set_quality(scene.quality) {
            resource.get_or_insert(None);
        }
        if self.seen.restarts != scene.restarts {
            self.seen.restarts = scene.restarts;
            self.simulation.restart();
            self.posed_at = None;
            resource = Some(Some(0));
        }
        if self.seen.paused != scene.liquid_paused {
            self.seen.paused = scene.liquid_paused;
            self.liquid.pause(scene.liquid_paused);
        }
        if self.seen.recomputes != scene.recomputes {
            self.seen.recomputes = scene.recomputes;
            self.liquid.recompute();
        }
        if !Arc::ptr_eq(&self.seen.wind, &scene.wind) || self.seen.drafts != scene.drafts {
            self.seen.wind = Arc::clone(&scene.wind);
            self.seen.drafts = scene.drafts.clone();
            self.simulation.wake();
        }
        if let Some(since) = resource {
            self.update_liquid_scene(scene, since);
        }
    }

    pub(super) fn update_liquid_scene(&mut self, scene: &Scene, hint: Option<u32>) {
        let mut figures: Vec<CastMember> = scene.members.clone();
        figures.sort_by_key(|member| member.id);
        let built = Arc::new(LiquidScene::new(
            figures,
            Arc::clone(&scene.liquids),
            Arc::clone(&scene.wind),
            self.simulation.cloth_scene(),
        ));
        let since = [hint, built.changed_from(&self.liquid_scene)]
            .into_iter()
            .flatten()
            .min();
        self.liquid_scene = built;
        self.liquid.set_source(&self.scene_source(), since);
    }

    pub(super) fn prepare(&mut self, scene: &Scene) {
        for member in &scene.members {
            if !self.gpus.contains_key(&member.id) && !self.prepared.contains_key(&member.id) {
                let prepared = Prepared::new(Arc::clone(&member.model), &UploadOptions::default());
                self.prepared.insert(member.id, prepared);
            }
        }
    }

    pub(super) fn upload(&mut self, renderer: &mut Renderer, scene: &Scene) {
        for member in &scene.members {
            if self.gpus.contains_key(&member.id) || self.unloadable.contains(&member.id) {
                continue;
            }
            let Some(prepared) = self.prepared.remove(&member.id) else {
                continue;
            };
            match GpuModel::upload_prepared(renderer, prepared) {
                Ok(gpu) => {
                    let mut actor = Actor::cast(member.clone(), gpu.instance());
                    actor.hold(scene.pending.as_ref());
                    self.actors.push(actor);
                    self.gpus.insert(member.id, gpu);
                    self.simulation.recast();
                    self.posed_at = None;
                    self.dirty = true;
                }
                Err(error) => {
                    self.unloadable.insert(member.id);
                    self.fail(StageError::Upload(error));
                }
            }
        }
    }

    pub(super) fn draw(
        &mut self,
        mut input: Input,
        ctx: SceneCtx<'_>,
        now: Instant,
    ) -> (Option<Frame>, Published) {
        let SceneCtx {
            renderer,
            width,
            height,
        } = ctx;
        self.sync(&mut input);
        let input = input;
        self.upload(renderer, &input.scene);
        self.render_captures(renderer, &input.scene);
        let mut frame = None;
        if self.pacer.is_due(now) {
            frame = self.build(
                &input,
                SceneCtx {
                    renderer,
                    width,
                    height,
                },
                now,
            );
            if frame.is_some() {
                self.pacer.drew(now);
            }
        }
        if input.wants_snapshot {
            self.snapshot_cloth();
        }
        let drew = frame.is_some();
        (frame, self.published(&input, now, drew))
    }

    fn published(&mut self, input: &Input, now: Instant, drew: bool) -> Published {
        if let Some(error) = self.liquid.take_error() {
            self.errors.push(StageError::Liquid(error));
        }
        let graspable = self.simulation.is_clothed() && !self.cloth_from_liquid();
        let built = !self.dirty;
        let outputs = &mut self.outputs;
        if built {
            outputs.revision = input.revision;
        }
        outputs.restless = self.simulation.is_restless() || !self.captures.is_empty();
        outputs.posed_at = self.posed_at;
        outputs.size = self.size;
        outputs.due = Some(self.pacer.due(now));
        outputs.progress = self.liquid.progress();
        outputs.graspable = graspable;
        outputs.bounds = self
            .actors
            .iter()
            .filter(|actor| actor.visible)
            .map(|actor| (actor.id, actor.instance.bounds()))
            .collect();
        Published {
            outputs: Outputs {
                revision: outputs.revision,
                projection: outputs.projection.clone(),
                aiming: outputs.aiming.clone(),
                emitters: outputs.emitters.clone(),
                vane: outputs.vane,
                paint_surface: outputs.paint_surface.clone(),
                cloth_snapshot: outputs.cloth_snapshot.clone(),
                graspable: outputs.graspable,
                restless: outputs.restless,
                posed_at: outputs.posed_at,
                size: outputs.size,
                due: outputs.due,
                bounds: outputs.bounds.clone(),
                progress: outputs.progress.clone(),
            },
            errors: std::mem::take(&mut self.errors),
            drew,
        }
    }

    fn build(&mut self, input: &Input, ctx: SceneCtx<'_>, now: Instant) -> Option<Frame> {
        let aspect = ctx.aspect();
        let SceneCtx {
            renderer,
            width,
            height,
        } = ctx;
        let scene = &input.scene;
        let size = PixelSize::new(width, height);
        if self.size != size {
            self.size = size;
            self.dirty = true;
        }
        let (time, running) = (input.time, input.running);
        self.liquid.set_target_step(step_of(input.length.max(time)));
        if self.liquid.is_stale(step_of(time)) {
            self.dirty = true;
        }
        self.request_cloth_assets(scene);
        let moved = self.posed_at != Some(time);
        if moved {
            self.simulation.wake();
        }
        if moved || self.simulation.is_restless() {
            self.step_physics(renderer, scene, time, running, now);
            self.posed_at = Some(time);
            self.dirty = true;
        }
        let roused = self.dressing.take_roused();
        if !std::mem::take(&mut self.dirty) && !roused {
            return None;
        }

        let shot = scene.shot(time, running);
        let camera = shot.camera(aspect);
        let mut projection = scene
            .focus
            .and_then(|focus| self.actors.iter().find(|actor| actor.id == focus))
            .map(|actor| actor.projection(camera.view_projection()));
        if let Some(projection) = &mut projection {
            self.project_grips(projection, time);
        }
        self.outputs.projection = projection;
        self.outputs.aiming = self.aim_emitter(scene, camera.view_projection(), time, running);
        self.outputs.vane = self.project_wind(scene, camera.view_projection(), shot.target(), time);
        self.outputs.paint_surface =
            self.paint_surface_at(scene, camera.view_projection(), camera.position());
        let view_projection = camera.view_projection();
        let mut frame = self.compose(renderer, scene, camera, time);
        let marks = self.dressing.marks();
        self.outputs.emitters = (!marks.is_empty()).then(|| {
            Arc::new(EmitterMarks {
                view_projection,
                marks: marks.to_vec(),
            })
        });
        if self
            .drawn_at
            .is_some_and(|before| time < before || time - before > LONGEST_STEP)
        {
            frame.cut();
        }
        self.drawn_at = Some(time);
        Some(frame)
    }

    pub(super) fn pose_at(&mut self, renderer: &Renderer, scene: &Scene, time: f32) {
        self.request_cloth_assets(scene);
        for lead_time in self.simulation.lead_in(time) {
            self.step_physics(renderer, scene, lead_time, true, Instant::now());
        }
        self.posed_at = None;
    }

    fn request_cloth_assets(&mut self, scene: &Scene) {
        let mut cloth_assets_dropped = false;
        for at in 0..self.actors.len() {
            let id = self.actors[at].id;
            if !scene.has_enabled_cloth(id) {
                if self.simulation.has_cloth_asset(id) {
                    self.simulation.forget(id);
                    cloth_assets_dropped = true;
                }
                continue;
            }
            let Some(figure_cloth) = scene.figure_cloth.get(&id) else {
                continue;
            };
            let Some(member) = scene.members.iter().find(|member| member.id == id) else {
                continue;
            };
            if let Err(error) = self.simulation.want(
                id,
                &member.model,
                &member.anatomy,
                &figure_cloth.cloth_settings,
                figure_cloth.revision,
            ) {
                self.fail(StageError::Cloth(error));
            }
        }
        if cloth_assets_dropped {
            self.update_liquid_scene(scene, None);
        }
    }

    fn step_physics(
        &mut self,
        renderer: &Renderer,
        scene: &Scene,
        time: f32,
        running: bool,
        now: Instant,
    ) {
        let input = StepInput {
            time,
            running,
            now,
            stepping: !self.cloth_from_liquid(),
            wind: scene.wind_at(time),
        };
        let output = self
            .simulation
            .step(&mut self.actors, renderer.gpu().compute(), input);
        if let Some(error) = output.error {
            self.fail(StageError::Cloth(error));
        }
        if output.cloth_changed {
            self.update_liquid_scene(scene, None);
        }
    }

    pub(super) fn cloth_from_liquid(&self) -> bool {
        matches!(self.liquid, Playback::Simulated(_))
            && !SceneSource::emitters(self.liquid_scene.as_ref()).is_empty()
    }

    fn pour(&mut self, renderer: &Renderer, time: f32) -> Option<SceneFrame> {
        let compute = renderer.gpu().compute().ok()?;
        let scene_source = self.scene_source();
        match self.liquid.frame(&compute, &scene_source, step_of(time)) {
            Ok(scene_frame) => scene_frame,
            Err(error) => {
                self.fail(StageError::Liquid(error));
                None
            }
        }
    }

    pub(super) fn compose(
        &mut self,
        renderer: &mut Renderer,
        scene: &Scene,
        camera: Camera,
        time: f32,
    ) -> Frame {
        let mut frame = Frame::new(camera);
        frame.time(time).outline(Outline {
            enabled: false,
            ..Outline::default()
        });
        let scene_frame = self.pour(renderer, time);
        let wets = matches!(scene.liquids.settings.wetness, Wetness::On { .. });
        let players: Vec<Player<'_>> = self
            .actors
            .iter()
            .enumerate()
            .filter_map(|(at, actor)| {
                if !actor.visible || !self.gpus.contains_key(&actor.id) {
                    return None;
                }
                Some(Player {
                    id: actor.id,
                    name: &actor.name,
                    look_name: actor.look_name,
                    head: actor.head.map(|head| head.place(&actor.instance)),
                    instance: &actor.instance,
                    focused: scene.focus == Some(actor.id),
                    channel: figure_channel(at),
                })
            })
            .collect();
        let staged = self.dressing.stage(
            renderer,
            &mut frame,
            &scene.look,
            Moment {
                camera: &camera,
                time,
                players: &players,
            },
            &mut self.gpus,
        );
        let mut failure = None;
        for player in &players {
            let Some(gpu) = self.gpus.get(&player.id) else {
                continue;
            };
            if let Err(error) = gpu.refresh_materials(renderer, player.instance) {
                failure = Some(StageError::Upload(error));
            }
            let figure = staged.figures.get(&player.id).copied();
            let baked = scene_frame
                .as_ref()
                .filter(|_| self.simulation.is_enabled())
                .map(|scene_frame| scene_frame.drapes(&mut frame, FigureKey(player.id.number())))
                .filter(|drapes| !drapes.is_empty());
            let drapes = baked.unwrap_or_else(|| self.simulation.drapes(&mut frame, player.id));
            let drawing = gpu
                .drawing(player.instance)
                .light_channels(DEFAULT_LIGHT_CHANNELS | player.channel);
            let drawing = match figure {
                Some(figure) => drawing.figure(figure),
                None => drawing,
            };
            let drawing = match &scene.highlight {
                Some((figure, materials)) if *figure == player.id => drawing.selected(materials),
                _ => drawing,
            };
            let drawing = drapes
                .into_iter()
                .fold(drawing, |drawing, (node, drape)| drawing.drape(node, drape));
            let wetness = scene_frame
                .as_ref()
                .filter(|_| wets)
                .and_then(|scene_frame| {
                    scene_frame.wetness(&mut frame, FigureKey(player.id.number()))
                });
            match wetness {
                Some(wetness) => drawing.wet(wetness).draw_into(&mut frame),
                None => drawing.draw_into(&mut frame),
            }
        }
        if let Some(reason) = self.dressing.take_failure() {
            failure = Some(StageError::Look(reason));
        }
        if let Some(error) = failure {
            self.fail(error);
        }
        if let Some(scene_frame) = &scene_frame {
            scene_frame.draw_fluid(&mut frame, self.liquid_scene.materials());
        }
        frame
    }
}

impl std::fmt::Debug for Render {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Render")
            .field("actors", &self.actors.len())
            .field("gpus", &self.gpus.len())
            .finish_non_exhaustive()
    }
}
