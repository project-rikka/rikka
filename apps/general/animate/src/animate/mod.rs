mod baking;
mod booth;
mod cloth;
mod curves;
mod edit;
mod editing;
mod exporting;
mod focusing;
mod holding;
mod importing;
mod library;
mod liquids;
mod look;
mod morphing;
mod placing;
mod posing;
mod pouring;
mod props;
mod saving;
mod serving;
mod settings;
mod shaping;
mod subject_motion;
mod subjects;
mod timing;
mod wind;
mod workshop;

use animate_inspector::EmitterNaming;
use animate_library_browser::Shelving;
use animate_shader_editor::{ShaderEditor, Workshop};
use curves::Curves;
use edit::{Edit, Pair};
pub(crate) use importing::Choosing;
use library::Shelves;
use subject_motion::SubjectMotion;

pub(crate) use look::{LookPart, Starter};
pub(crate) use serving::ServeError;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use animate_body::Body;
use animate_cast::{Cast, Eye, FigureId, Mode, PropId, PropRole, Subject, TakePlayback, time_of};
use animate_document::Motion;
use animate_edit::{Channel, Clip, History};
use animate_export::Job;
use animate_fluid::{EmitterId, FluidSettings};
use animate_inspector::{Controls, Studio, StudioCore};
use animate_look::world::Sampling;
use animate_look::{Plans, shared_library};
use animate_motion::{Incoming, Take};
use animate_preferences::Preferences;
use animate_shader::{SharedCompiler, SharedShaders, Workbench};
use animate_stage::{Dressing, Render, SharedRender, SharedStage, Stage};
use animate_timeline::Timeline;
use animate_transport::{PlaybackState, Transport};
use rikka_app::prelude::*;
use rikka_assist::ToolRegistration;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_renderer_3d::ViewportConfig;
use rikka_sync::Mutex;
use rikka_ui::{DragOffer, StateAccess};
use rikka_viewport::Viewport3d;
use rustc_hash::FxHashSet;

use crate::arrival::{
    self, Arrival, FigureTarget, LoadRequest, LoadTarget, Placement, PropTarget, Settled,
};
use crate::output::Output;
use crate::{assistant, wording};

const TICKING: &str = "animate.ticking";
const TICK: Duration = Duration::from_millis(30);
const RESTING_SPLIT: f32 = 0.62;
const RESTING_DOCK: f32 = 0.5;
const ZOOM_STEP: f64 = 1.5;
const SOUNDS: [&str; 9] = [
    "audio/mpeg",
    "audio/wav",
    "audio/x-wav",
    "audio/vnd.wave",
    "audio/flac",
    "audio/ogg",
    "audio/x-vorbis+ogg",
    "audio/mp4",
    "audio/aac",
];
const FIGURES: [&str; 2] = ["model/x.pmx", "model/vrm"];
const MOTIONS: [&str; 2] = ["model/x.vmd", "model/x.vrma"];
const SCENES: [&str; 3] = ["model/x.fbx", "model/gltf-binary", "model/gltf+json"];

#[derive(Debug, Default)]
struct Loading {
    queue: VecDeque<Batch>,
    choices: VecDeque<Choosing>,
    busy: bool,
    opening: Vec<PathBuf>,
}

#[derive(Debug)]
struct Batch {
    paths: Vec<PathBuf>,
    settled: Option<Settled>,
    placement: Placement,
}

#[derive(Debug, Default)]
struct Morphing {
    query: TextDocument,
    sides: FxHashSet<String>,
    typing: Option<Vec<String>>,
    entry: TextDocument,
}

#[derive(Debug)]
struct Pouring {
    settings: FluidSettings,
    computing: bool,
    naming: TextDocument,
    named: Option<(EmitterId, String)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DockTab {
    #[default]
    Timeline,
    Nodes,
}

#[derive(Debug)]
pub(crate) struct Dock {
    pub(crate) tab: DockTab,
    pub(crate) split: bool,
    pub(crate) ratio: Ratio,
}

impl Dock {
    pub(crate) fn shows_nodes(&self) -> bool {
        self.split || self.tab == DockTab::Nodes
    }
}

#[derive(Debug, State)]
pub(crate) struct Animate {
    cast: Cast,
    subject: Option<Subject>,
    transport: Transport,
    timeline: Timeline,
    stage: SharedStage,
    render: SharedRender,
    viewport: Viewport3d,
    split: Ratio,
    loading: Loading,
    notice: Option<Label>,
    folder: Option<PathBuf>,
    physics: bool,
    history: History<Subject, Edit>,
    undo_group: Option<u64>,
    clip: Option<Clip>,
    curves: Curves,
    channel: Channel,
    inspecting: bool,
    output: Output,
    mode: Mode,
    controls: Controls,
    sender: Sender<Animate>,
    seat: Option<ToolRegistration>,
    undo_groups: u64,
    job: Option<Job>,
    pouring: Pouring,
    preferences: Preferences,
    morphing: Morphing,
    workbench: Workbench,
    shaders: SharedShaders,
    compiler: SharedCompiler,
    look_plans: Plans,
    sampling: Sampling,
    shader: ShaderEditor,
    dock: Dock,
    preparing: FxHashSet<PropId>,
    shelves: Shelves,
}

impl Animate {
    pub(crate) fn new(
        opening: Vec<PathBuf>,
        preferences: Preferences,
        sender: Sender<Animate>,
    ) -> Self {
        let transport = Transport::default();
        let physics = true;
        let shaders = SharedShaders::default();
        let compiler = SharedCompiler::default();
        let stage: SharedStage = Arc::new(Mutex::new(Stage::new(
            transport.clock(),
            physics,
            preferences.cloth_quality(),
        )));
        let mut built = Render::new(
            physics,
            Dressing::new(Arc::clone(&shaders), compiler.clone()),
        );
        let notify = baking::notifier(&sender);
        built.notify_with(Arc::clone(&notify));
        match animate_stage::bake(notify) {
            Ok(bake) => built.attach_bake(bake),
            Err(error) => tracing::warn!(%error, "cannot start liquid bake"),
        }
        let render: SharedRender = Arc::new(Mutex::new(built));
        let sampling = Sampling::default();
        let viewport = Viewport3d::with_config(live_viewport());
        let render_stage = Arc::clone(&stage);
        let drawing = Arc::clone(&render);
        viewport.set_scene(move |ctx| animate_stage::draw(&render_stage, &drawing, ctx));
        let (shelves, problems) = Shelves::open(&preferences, &sender);
        Self {
            cast: Cast::default(),
            subject: None,
            transport,
            timeline: Timeline::default(),
            stage,
            render,
            viewport,
            split: Ratio::new(RESTING_SPLIT),
            loading: Loading {
                opening,
                ..Loading::default()
            },
            notice: (!problems.is_empty()).then(|| Label::from(problems.join("\n"))),
            folder: None,
            physics,
            history: History::default(),
            undo_group: None,
            clip: None,
            curves: Curves::default(),
            channel: Channel::X,
            inspecting: true,
            mode: Mode::default(),
            controls: Controls::default(),
            output: Output::new(preferences.clone()),
            sender,
            seat: None,
            undo_groups: 0,
            job: None,
            pouring: Pouring {
                settings: preferences.liquid(),
                computing: true,
                naming: TextDocument::default(),
                named: None,
            },
            preferences,
            morphing: Morphing::default(),
            workbench: Workbench::new(Arc::new(shared_library().clone())),
            shaders,
            compiler,
            look_plans: Plans::default(),
            sampling,
            shader: ShaderEditor::default(),
            dock: Dock {
                tab: DockTab::default(),
                split: false,
                ratio: Ratio::new(RESTING_DOCK),
            },
            preparing: FxHashSet::default(),
            shelves,
        }
    }

    pub(crate) fn mode(&self) -> Mode {
        self.mode
    }

    pub(crate) fn set_mode(&mut self, mode: Mode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.controls.card = None;
        self.cast.resheet(mode);
        self.timeline.pick(Default::default());
        self.stage.lock().release();
        self.refresh();
    }

    pub(crate) fn switch_mode(&mut self) {
        self.set_mode(self.mode.toggled());
    }
    pub(crate) fn controls(&self) -> &Controls {
        &self.controls
    }

    pub(crate) fn controls_mut(&mut self) -> &mut Controls {
        &mut self.controls
    }

    pub(crate) fn is_easy_mode(&self) -> bool {
        self.mode == Mode::Easy && self.chosen_body().is_some()
    }

    pub(crate) fn chosen_body(&self) -> Option<&Arc<Body>> {
        let Some(Subject::Figure(id)) = self.subject else {
            return None;
        };
        self.cast.figure(id)?.body.as_ref()
    }

    fn eye(&self) -> Eye {
        Eye::new(self.camera_motion())
    }

    pub(crate) fn set_take(&mut self, id: FigureId, take: Arc<Take>) {
        let eye = self.eye();
        let mode = self.mode;
        let Some(figure) = self.cast.figure_mut(id) else {
            return;
        };
        let playback = Arc::new(match &figure.playback {
            Some(playback) => {
                playback.rebind(&figure.model, &figure.take, &take, figure.performer(), &eye)
            }
            None => TakePlayback::bind(&figure.model, &take, figure.performer(), &eye),
        });
        let before = std::mem::replace(&mut figure.take, take);
        let changed = figure.take.changed_from(&before);
        figure.playback = Some(Arc::clone(&playback));
        figure.retake(&before, mode);
        self.stage.lock().pose(id, Some(playback), changed);
    }

    fn rebake_gaze(&mut self) {
        let watching: Vec<FigureId> = self
            .cast
            .figures()
            .iter()
            .filter(|figure| figure.is_human() && figure.watches_camera())
            .map(|figure| figure.id)
            .collect();
        let eye = self.eye();
        for id in watching {
            let Some(figure) = self.cast.figure_mut(id) else {
                continue;
            };
            let Some(playback) = &figure.playback else {
                continue;
            };
            let playback = Arc::new(playback.rebaked(&figure.take, figure.performer(), &eye));
            figure.playback = Some(Arc::clone(&playback));
            self.stage.lock().pose(id, Some(playback), Some(0));
        }
    }

    pub(crate) fn cast(&self) -> &Cast {
        &self.cast
    }

    pub(crate) fn subject(&self) -> Option<Subject> {
        self.subject
    }

    pub(crate) fn transport(&self) -> &Transport {
        &self.transport
    }

    pub(crate) fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    pub(crate) fn stage(&self) -> &SharedStage {
        &self.stage
    }

    pub(crate) fn viewport(&self) -> &Viewport3d {
        &self.viewport
    }

    pub(crate) fn output(&self) -> &Output {
        &self.output
    }

    pub(crate) fn output_mut(&mut self) -> &mut Output {
        &mut self.output
    }

    pub(crate) fn split(&self) -> &Ratio {
        &self.split
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.loading.busy
    }

    pub(crate) fn notice(&self) -> Option<&Label> {
        self.notice.as_ref()
    }

    pub(crate) fn has_camera(&self) -> bool {
        self.cast.camera().is_some()
    }

    pub(crate) fn start(&mut self) {
        self.refresh();
        let opening = std::mem::take(&mut self.loading.opening);
        self.load(opening);
        self.offer();
    }

    fn offer(&mut self) {
        self.seat = assistant::seat(self.sender.clone(), self.seat_title());
    }

    fn seat_title(&self) -> String {
        let figures = self.cast.figures();
        match figures.is_empty() {
            true => "Empty scene".to_owned(),
            false => figures
                .iter()
                .map(|figure| figure.name.as_str())
                .collect::<Vec<&str>>()
                .join(", "),
        }
    }

    pub(crate) fn focused(&mut self) {
        if let Some(seat) = &self.seat {
            seat.mark_active();
        }
        skip_view();
    }

    pub(crate) fn open_assistant(&mut self) {
        assistant::open_assistant();
        skip_view();
    }

    pub(crate) fn load(&mut self, paths: Vec<PathBuf>) {
        self.load_as(paths, Placement::Figure);
    }

    pub(crate) fn dropped(offer: DragOffer) {
        spawn_blocking(
            move || {
                offer
                    .uris()
                    .iter()
                    .filter_map(|uri| rikka_open::uri_to_path(uri))
                    .collect::<Vec<PathBuf>>()
            },
            |animate: &mut Animate, paths| animate.load(paths),
        );
    }

    pub(crate) fn add(&self) {
        let sounds = SOUNDS.into_iter().fold(
            Filter::named(tr!("animate.add.sound_files").to_string()),
            Filter::mime_type,
        );
        let everything = SOUNDS
            .into_iter()
            .chain(FIGURES)
            .chain(SCENES)
            .chain(MOTIONS)
            .chain([Motion::MIME])
            .fold(
                Filter::named(tr!("animate.add.everything").to_string()),
                Filter::mime_type,
            );
        let figures = FIGURES.into_iter().chain(SCENES).fold(
            Filter::named(tr!("animate.add.figure_files").to_string()),
            Filter::mime_type,
        );
        let motions = MOTIONS.into_iter().chain(SCENES).fold(
            Filter::named(tr!("animate.add.motion_files").to_string()),
            Filter::mime_type,
        );
        let chooser = Chooser::open(tr!("animate.add.title").to_string())
            .parent(window_identifier())
            .accept_label(tr!("animate.add.accept").to_string())
            .multiple(true)
            .filter(everything)
            .filter(figures)
            .filter(motions)
            .filter(
                Filter::named(tr!("animate.add.rikka_motion_files").to_string())
                    .mime_type(Motion::MIME),
            )
            .filter(sounds);
        let chooser = match &self.folder {
            Some(folder) => chooser.current_folder(folder),
            None => chooser,
        };
        spawn_blocking(
            move || chooser.run(),
            |animate: &mut Animate, chosen: rikka_dialog::Result<ChooserResponse>| match chosen {
                Ok(ChooserResponse::Files(paths)) => {
                    animate.folder = paths
                        .first()
                        .and_then(|path| path.parent())
                        .map(PathBuf::from);
                    animate.load(paths);
                }
                Ok(ChooserResponse::Cancelled) => skip_view(),
                Err(error) => animate.notice = Some(Label::from(error.to_string())),
            },
        );
    }

    fn next_load(&mut self) {
        if self.loading.busy {
            return;
        }
        let Some(batch) = self.loading.queue.pop_front() else {
            return;
        };
        self.loading.busy = true;
        let order = LoadRequest {
            paths: batch.paths,
            settled: batch.settled,
            placement: batch.placement,
            target: self.load_target(),
            camera: self.camera_motion(),
            emitter: self.chosen_emitter().map(|emitter| emitter.id),
            uploads: self.stage.lock().uploads(),
        };
        spawn_blocking(
            move || arrival::load(order),
            |animate: &mut Animate, arrivals| animate.on_loaded(arrivals),
        );
    }

    fn load_target(&self) -> LoadTarget {
        match self.subject {
            Some(Subject::Figure(id)) => {
                self.cast.figure(id).map_or(LoadTarget::Camera, |figure| {
                    LoadTarget::Figure(FigureTarget::from_figure(figure))
                })
            }
            Some(subject @ (Subject::Stage | Subject::Accessory(_))) => {
                self.cast.prop(subject).map_or(LoadTarget::Camera, |track| {
                    LoadTarget::Prop(PropTarget::from_prop(&track.prop))
                })
            }
            _ => LoadTarget::Camera,
        }
    }

    pub(crate) fn on_loaded(&mut self, arrivals: Vec<Arrival>) {
        self.loading.busy = false;
        for arrival in arrivals {
            self.accept(arrival);
        }
        self.refresh();
        self.next_load();
    }

    fn accept(&mut self, arrival: Arrival) {
        match arrival {
            Arrival::Figure {
                mut figure,
                prepared,
            } => {
                let id = figure.id;
                figure.resheet(self.mode);
                self.stage.lock().add_figure(&figure, prepared);
                self.cast.add_figure(figure);
                self.load_cloth(id);
                self.survey(id);
                self.subject = Some(Subject::Figure(id));
            }
            Arrival::Prop { prop, prepared } => self.accept_prop(prop, prepared),
            Arrival::Motion {
                figure,
                onto,
                incoming,
                take,
                playback,
            } => self.land_take(figure, &onto, &incoming, take, playback),
            Arrival::PropMotion {
                role,
                onto,
                incoming,
                take,
            } => self.land_prop_take(role, &onto, &incoming, take),
            Arrival::Camera {
                onto,
                incoming,
                motion,
            } => {
                let live = self.camera_motion();
                let after = match (&live, &onto) {
                    (Some(live), Some(onto)) if Arc::ptr_eq(live, onto) => motion,
                    (None, None) => motion,
                    (live, _) => {
                        Arc::new(incoming.onto(&live.as_deref().cloned().unwrap_or_default()))
                    }
                };
                self.replace(Subject::Camera, Some(SubjectMotion::Camera(after)));
            }
            Arrival::Sound(sound) => {
                self.transport.set_soundtrack(Some(sound));
            }
            Arrival::Look(look) => self.replace(Subject::Look, Some(SubjectMotion::Look(look))),
            Arrival::Emitter { emitter, replacing } => self.add_loaded_emitter(*emitter, replacing),
            Arrival::Wind(wind) => self.replace(Subject::Wind, Some(SubjectMotion::Wind(wind))),
            Arrival::Placed { file, kind, record } => self.land_placement(&file, kind, *record),
            Arrival::Choose(choice) => self.choose(*choice),
            Arrival::Rejected { file, rejection } => {
                self.notice = Some(wording::rejection(&file, &rejection));
            }
        }
    }

    fn land_take(
        &mut self,
        id: FigureId,
        onto: &Arc<Take>,
        incoming: &Incoming<Take>,
        take: Arc<Take>,
        playback: Arc<TakePlayback>,
    ) {
        let mode = self.mode;
        let Some(figure) = self.cast.figure_mut(id) else {
            return;
        };
        let before = Arc::clone(&figure.take);
        let after = match Arc::ptr_eq(&before, onto) {
            true => {
                figure.take = Arc::clone(&take);
                figure.playback = Some(Arc::clone(&playback));
                figure.resheet(mode);
                self.stage.lock().pose(id, Some(playback), Some(0));
                take
            }
            false => {
                let take = Arc::new(incoming.onto(&before));
                self.set_take(id, Arc::clone(&take));
                take
            }
        };
        self.stage.lock().reset_physics();
        self.record(
            Subject::Figure(id),
            Edit::Replaced(Pair::new(
                Some(SubjectMotion::Take(before)),
                Some(SubjectMotion::Take(after)),
            )),
        );
    }

    fn relength(&mut self) {
        let motion = time_of(self.cast.last_key());
        let sound = self.transport.sound_length().unwrap_or(0.0);
        self.transport.set_length(motion.max(sound));
        self.timeline.set_length(self.transport.last_frame());
    }

    fn refresh(&mut self) {
        self.relength();
        self.timeline.set_peaks(self.transport.peaks());
        if self.subject.is_none() {
            self.subject = self.cast.subjects().first().copied();
        }
        let subject = self.subject;
        self.timeline.show(
            subject,
            subject.map_or_else(Arc::default, |subject| self.cast.sheet(subject)),
        );
        let mut stage = self.stage.lock();
        stage.follow_camera(subject == Some(Subject::Camera));
        stage.show_wind(subject == Some(Subject::Wind));
        stage.show_prop(subject.and_then(PropRole::of));
        stage.focus(match subject {
            Some(Subject::Figure(id)) => Some(id),
            Some(
                Subject::Stage
                | Subject::Accessory(_)
                | Subject::Emitter(_)
                | Subject::Camera
                | Subject::Look
                | Subject::Wind,
            )
            | None => None,
        });
        stage.set_look(Arc::clone(&self.cast.look().motion));
        stage.set_wind(Arc::clone(&self.cast.wind().motion));
        drop(stage);
        self.sync_props();
        self.sync_shaders();
        self.follow_sampling();
        self.fit_cloth();
        self.pour_liquids();
        self.sync_naming();
        if let Some(seat) = &self.seat {
            seat.set_title(self.seat_title());
        }
    }

    pub(crate) fn dismiss_notice(&mut self) {
        self.notice = None;
    }

    pub(crate) fn choose_subject(&mut self, index: usize) {
        let Some(subject) = self.cast.subjects().get(index).copied() else {
            return;
        };
        match self.subject.replace(subject) {
            Some(Subject::Figure(left)) => self.stage.lock().highlight(left, &[]),
            Some(left @ (Subject::Stage | Subject::Accessory(_))) => {
                if let Some(track) = self.cast.prop(left) {
                    self.stage.lock().highlight_prop(track.prop.id, &[]);
                }
            }
            _ => {}
        }
        self.controls.card = None;
        self.refresh();
    }

    pub(crate) fn remove_subject(&mut self) {
        let Some(subject) = self.subject else {
            return;
        };
        if let Subject::Emitter(_) | Subject::Stage | Subject::Accessory(_) = subject {
            self.replace(subject, None);
            return;
        }
        if subject == Subject::Look {
            self.in_undo_group(|animate| {
                animate.reset_look(LookPart::World, Starter::Bare);
                animate.reset_look(LookPart::Composite, Starter::Bare);
            });
            return;
        }
        self.history.forget(subject);
        match subject {
            Subject::Figure(id) => {
                self.subject = self.cast.dismiss(id);
                self.stage.lock().dismiss(id);
                self.refresh();
            }
            subject => self.put(subject, None),
        }
    }

    pub(crate) fn clear_motion(&mut self) {
        let id = match self.subject {
            Some(Subject::Figure(id)) => id,
            Some(subject) if PropRole::of(subject).is_some() => {
                self.clear_prop_motion(subject);
                return;
            }
            _ => return,
        };
        let Some(figure) = self.cast.figure_mut(id) else {
            return;
        };
        figure.take = Arc::default();
        figure.playback = None;
        figure.resheet(self.mode);
        self.history.forget(Subject::Figure(id));
        {
            let mut stage = self.stage.lock();
            stage.pose(id, None, Some(0));
            stage.reset_physics();
        }
        self.refresh();
    }

    pub(crate) fn clear_sound(&mut self) {
        self.transport.set_soundtrack(None);
        self.refresh();
    }

    pub(crate) fn play_pause(&mut self) {
        self.transport.toggle();
        if self.transport.is_playing() {
            every_keyed(TICKING, TICK, |animate: &mut Animate| animate.on_tick());
        }
    }

    fn on_tick(&mut self) {
        match self.transport.sync_to_sound() {
            PlaybackState::Running => self.timeline.follow(self.transport.frame()),
            PlaybackState::Ended => cancel(TICKING),
        }
    }

    pub(crate) fn step(&mut self, frames: i64) {
        self.transport.step(frames);
        self.timeline.follow(self.transport.frame());
    }

    pub(crate) fn go_to(&mut self, frame: u32) {
        self.transport.seek_frame(frame);
        self.timeline.follow(self.transport.frame());
    }

    pub(crate) fn go_to_end(&mut self) {
        self.go_to(self.transport.last_frame());
    }

    pub(crate) fn go_to_key(&mut self, forward: bool) {
        let Some(subject) = self.subject else {
            return;
        };
        let sheet = self.cast.sheet(subject);
        let frame = self.transport.frame();
        let target = match forward {
            true => sheet.next_key(frame),
            false => sheet.previous_key(frame),
        };
        if let Some(key) = target {
            if self.transport.is_playing() {
                self.transport.pause();
            }
            self.go_to(key);
        }
    }

    pub(crate) fn toggle_repeat(&mut self) {
        self.transport.set_repeat(!self.transport.repeats());
    }

    pub(crate) fn set_volume(&mut self, volume: f32) {
        self.transport.set_volume(volume);
    }

    pub(crate) fn zoom(&mut self, closer: bool) {
        self.timeline.zoom_by(match closer {
            true => ZOOM_STEP,
            false => ZOOM_STEP.recip(),
        });
        skip_view();
    }

    pub(crate) fn zoom_to_fit(&mut self) {
        self.timeline.fit();
        skip_view();
    }

    pub(crate) fn is_inspector_open(&self) -> bool {
        self.inspecting
    }

    pub(crate) fn toggle_inspector(&mut self) {
        self.inspecting = !self.inspecting;
    }

    pub(crate) fn is_physics_enabled(&self) -> bool {
        self.physics
    }

    pub(crate) fn set_physics(&mut self, physics: bool) {
        self.physics = physics;
        self.stage.lock().set_physics(physics);
    }

    pub(crate) fn reset_physics(&mut self) {
        self.stage.lock().reset_physics();
    }

    pub(crate) fn rest_view(&mut self) {
        self.stage.lock().rest();
    }

    pub(crate) fn frame_all(&mut self) {
        self.stage.lock().frame_all();
    }

    pub(crate) fn dock(&self) -> &Dock {
        &self.dock
    }

    pub(crate) fn toggle_node_dock(&mut self) {
        let shown = self.dock.shows_nodes();
        self.dock.split = false;
        self.dock.tab = match shown {
            true => DockTab::Timeline,
            false => DockTab::Nodes,
        };
    }

    pub(crate) fn show_dock(&mut self, tab: DockTab) {
        self.dock.tab = tab;
    }

    pub(crate) fn split_dock(&mut self) {
        self.dock.split = !self.dock.split;
    }
}

pub(crate) fn live_viewport() -> ViewportConfig {
    ViewportConfig::default().identity(false)
}

impl StudioCore for Animate {
    fn cast(&self) -> &Cast {
        Animate::cast(self)
    }

    fn chosen_body(&self) -> Option<&Arc<Body>> {
        Animate::chosen_body(self)
    }

    fn controls(&self) -> &Controls {
        Animate::controls(self)
    }

    fn controls_mut(&mut self) -> &mut Controls {
        Animate::controls_mut(self)
    }

    fn is_easy_mode(&self) -> bool {
        Animate::is_easy_mode(self)
    }

    fn is_physics_enabled(&self) -> bool {
        Animate::is_physics_enabled(self)
    }

    fn mode(&self) -> Mode {
        Animate::mode(self)
    }

    fn reset_physics(&mut self) {
        Animate::reset_physics(self);
    }

    fn set_physics(&mut self, physics: bool) {
        Animate::set_physics(self, physics);
    }

    fn subject(&self) -> Option<Subject> {
        Animate::subject(self)
    }

    fn transport(&self) -> &Transport {
        Animate::transport(self)
    }
}

pub(crate) fn studio<'a>(
    access: &'a mut StateAccess<'_>,
) -> Option<&'a mut (dyn Studio + 'static)> {
    access
        .get::<Animate>()
        .map(|animate| animate as &mut (dyn Studio + 'static))
}

pub(crate) fn shelving<'a>(
    access: &'a mut StateAccess<'_>,
) -> Option<&'a mut (dyn Shelving + 'static)> {
    access
        .get::<Animate>()
        .map(|animate| animate as &mut (dyn Shelving + 'static))
}

pub(crate) fn workshop<'a>(
    access: &'a mut StateAccess<'_>,
) -> Option<&'a mut (dyn Workshop + 'static)> {
    access
        .get::<Animate>()
        .map(|animate| animate as &mut (dyn Workshop + 'static))
}
