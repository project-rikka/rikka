use rikka_sync::MutexGuard;
use std::hash::Hash;
use std::time::{Duration, Instant};

use rikka_animation::FrameClock;
use rikka_renderer::{Present, RenderOutput, Renderer, Scene, TRIM_AFTER, TextureId};
use rustc_hash::FxHashMap;

use rikka_picture::{Pulse, RefreshRate, SharedGpu, Tick, Waking};
use rikka_ui::consume::{any_offscreen_live, register_wake};
use rikka_ui::{Rendering, RuntimeMessage, Ui};

use crate::painter::{PaintResult, Painter, Presenting};
use crate::resources::{ResourceFrame, Resources};
use crate::runner::{TaskRunner, TaskSink};
use crate::tasks::Tasks;
use crate::video::VideoPlayback;
use crate::viewports::Viewports;
use crate::wake_hint::WakeHint;

type Adjustment<W> = Box<dyn FnOnce(&mut W) + Send>;

pub struct Hosted<S, W> {
    pub ui: Ui,
    scene: Scene,
    resend: bool,
    surface: Option<W>,
    painting: bool,
    built: bool,
    settling: bool,
    animating: bool,
    drawable: bool,
    awaiting: bool,
    unshown: Option<Instant>,
    resting: Option<Instant>,
    clock: FrameClock,
    pulses: Vec<Pulse>,
    pulsing: Vec<Pulse>,
    waiting_on_surface: Vec<Adjustment<W>>,
    tasks: Tasks,
    sealing: Option<Presenting<S>>,
    presenting: Option<Presenting<S>>,
    pub state: S,
}

impl<S, W> Hosted<S, W> {
    pub fn attach(&mut self, surface: W) {
        let mut surface = surface;
        for adjust in self.waiting_on_surface.drain(..) {
            adjust(&mut surface);
        }
        self.surface = Some(surface);
    }

    pub fn drawable(&mut self, drawable: bool) {
        self.drawable = drawable;
    }

    pub fn is_drawable(&self) -> bool {
        self.drawable
    }

    pub fn frame_answered(&mut self) {
        self.awaiting = false;
        self.unshown = None;
        if self.pulsing.is_empty() {
            return;
        }
        let tick = Tick {
            at: Instant::now(),
            period: self
                .clock
                .refresh()
                .unwrap_or_else(RefreshRate::published_period),
        };
        for pulse in self.pulsing.drain(..) {
            pulse.beat(tick);
        }
    }

    pub fn presented(&mut self, at: Instant, refresh: Option<Duration>) {
        self.clock.presented(at, refresh);
    }

    pub fn awaits_frame(&self) -> bool {
        self.awaiting
    }

    pub fn is_painting(&self) -> bool {
        self.painting
    }

    pub fn present_anew(&mut self) {
        self.scene.damage_all();
        self.resend = true;
        self.frame_answered();
    }

    pub fn has_surface(&self) -> bool {
        self.surface.is_some() || self.painting
    }

    pub fn adjust(&mut self, adjust: impl FnOnce(&mut W) + Send + 'static) {
        match self.surface.as_mut() {
            Some(surface) => adjust(surface),
            None => self.waiting_on_surface.push(Box::new(adjust)),
        }
    }

    pub fn seals_with(&mut self, sealing: Presenting<S>) {
        self.sealing = Some(sealing);
    }

    pub fn presents_with(&mut self, presenting: Presenting<S>) {
        self.presenting = Some(presenting);
    }

    fn texture_updated(&mut self, texture: TextureId) {
        self.scene.notify_texture_updated(texture);
    }

    fn send<K: Eq + Hash + Clone + Send + 'static>(&mut self, key: &K, painter: &mut Painter<K, W>)
    where
        W: Present,
    {
        let mut frame = painter.spent_frame(key);
        match std::mem::take(&mut self.resend) {
            true => self.scene.take_whole_frame(&mut frame),
            false => self.scene.take_frame(&mut frame),
        }
        match painter.send_frame(key, frame) {
            true => self.built = true,
            false => self.resend = true,
        }
    }
}

impl<S: std::fmt::Debug, W> std::fmt::Debug for Hosted<S, W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hosted")
            .field("has_surface", &self.has_surface())
            .field("painting", &self.painting)
            .field("tasks", &self.tasks.len())
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

const LOOK_AGAIN: Duration = Duration::from_millis(8);

const IDLE_SHARE: f32 = 0.5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameOutcome {
    pub wake: WakeHint,
}

impl FrameOutcome {
    pub fn wants_another_frame(&self) -> bool {
        !self.wake.is_idle()
    }
}

pub struct Drawing<'a, S> {
    pub ui: &'a mut Ui,
    pub state: &'a mut S,
}

impl<S> std::fmt::Debug for Drawing<'_, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Drawing").finish_non_exhaustive()
    }
}

pub struct Embedder<K, S, W> {
    resources: Resources,
    viewports: Option<Viewports>,
    runner: TaskRunner<K>,
    hosted: FxHashMap<K, Hosted<S, W>>,
    painter: Option<Painter<K, W>>,
    finished: Vec<(K, Option<RenderOutput>)>,
    wake: Option<Waking>,
}

impl<K, S, W> Embedder<K, S, W>
where
    K: Eq + Hash + Clone + Send + 'static,
    W: Present,
{
    pub fn new(sink: impl TaskSink<K>) -> Self {
        Self {
            resources: Resources::new(),
            viewports: None,
            runner: TaskRunner::new(sink),
            hosted: FxHashMap::default(),
            painter: None,
            finished: Vec::new(),
            wake: None,
        }
    }

    pub fn plays_video(&mut self, videos: Box<dyn VideoPlayback>) {
        self.resources.plays_video(videos);
    }

    pub fn wakes_with(&mut self, wake: Waking) {
        register_wake(&wake);
        self.wake = Some(wake);
    }

    pub fn renderer(&mut self) -> Option<MutexGuard<'_, Renderer>> {
        Some(self.painter.as_ref()?.renderer())
    }

    pub fn renderer_or_try_init<E>(
        &mut self,
        build: impl FnOnce() -> Result<Renderer, E>,
    ) -> Result<MutexGuard<'_, Renderer>, E> {
        if self.painter.is_none() {
            let renderer = build()?;
            share_gpu(&renderer);
            self.painter = Some(Painter::start(renderer, self.wake.clone()));
        }
        Ok(self
            .painter
            .as_ref()
            .expect("runner was just started")
            .renderer())
    }

    pub fn insert(&mut self, key: K, ui: impl Into<Ui>, state: S) -> &mut Hosted<S, W> {
        self.hosted
            .entry(key)
            .insert_entry(Hosted {
                ui: ui.into(),
                scene: Scene::new(),
                resend: false,
                surface: None,
                painting: false,
                built: false,
                settling: false,
                animating: false,
                drawable: true,
                awaiting: false,
                unshown: None,
                resting: None,
                clock: FrameClock::new(),
                pulses: Vec::new(),
                pulsing: Vec::new(),
                waiting_on_surface: Vec::new(),
                tasks: Tasks::default(),
                sealing: None,
                presenting: None,
                state,
            })
            .into_mut()
    }

    pub fn remove(&mut self, key: &K) -> Option<S> {
        if let Some(painter) = self.painter.as_ref() {
            painter.stop(key);
        }
        while self.hosted.get(key).is_some_and(|hosted| hosted.painting) {
            let Some(painter) = self.painter.as_ref() else {
                break;
            };
            let Some(done) = painter.next_painted(key) else {
                break;
            };
            self.took(done);
        }
        self.finished.retain(|(finished, _)| finished != key);
        let mut hosted = self.hosted.remove(key)?;
        drop(hosted.surface.take());
        if let Some(painter) = self.painter.as_mut() {
            painter.retire(key);
        }
        Some(hosted.state)
    }

    pub fn get(&self, key: &K) -> Option<&Hosted<S, W>> {
        self.hosted.get(key)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut Hosted<S, W>> {
        self.hosted.get_mut(key)
    }

    pub fn contains(&self, key: &K) -> bool {
        self.hosted.contains_key(key)
    }

    pub fn is_empty(&self) -> bool {
        self.hosted.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &Hosted<S, W>)> {
        self.hosted.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut Hosted<S, W>)> {
        self.hosted.iter_mut()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Hosted<S, W>> {
        self.hosted.values_mut()
    }

    pub fn frame(&mut self, drawing: impl FnMut(Drawing<'_, S>)) -> FrameOutcome {
        let now = Instant::now();
        let pace = RefreshRate::published_period();
        for (key, output) in self.collect_painted() {
            if let Some(entry) = self.hosted.get_mut(&key)
                && !output.is_some_and(|output| output.rendered())
            {
                entry.unshown = Some(now + pace);
            }
        }
        for entry in self.hosted.values_mut() {
            if entry.unshown.is_some_and(|due| due <= now) {
                entry.frame_answered();
            }
            if entry.resting.is_some_and(|due| due <= now) {
                entry.resting = None;
                if let Some(surface) = entry.surface.as_mut() {
                    surface.trim();
                }
            }
        }
        self.spawn_tasks();
        let resources = self.commit_resources();
        self.follow_viewports();
        let animating = self.draw_dirty(pace, drawing);
        let prefetching = self.prefetch_while_idle(pace);
        let unheard = self.wake.is_none() && self.in_flight();
        let paced = !self.hosted.is_empty() && self.hosted.values().all(|entry| entry.awaiting);
        let unshown = self.unshown_wake();
        FrameOutcome {
            wake: match paced && !unheard {
                true => unshown,
                false => wake_hint(animating || unheard || prefetching, &resources)
                    .soonest(self.next_wake())
                    .soonest(unshown),
            },
        }
    }

    fn prefetch_while_idle(&mut self, pace: Duration) -> bool {
        let until = Instant::now() + pace.mul_f32(IDLE_SHARE);
        let mut more = false;
        for entry in self.hosted.values_mut() {
            if entry.painting || !entry.drawable || !entry.ui.wants_prefetch() {
                continue;
            }
            if !entry.awaiting && entry.ui.needs_render() {
                more = true;
                continue;
            }
            if Instant::now() >= until {
                more = true;
                break;
            }
            entry.ui.prefetch(&mut entry.scene, until);
            more |= entry.ui.wants_prefetch() || entry.ui.needs_render();
        }
        more
    }

    fn unshown_wake(&self) -> WakeHint {
        let now = Instant::now();
        self.hosted
            .values()
            .flat_map(|entry| entry.unshown.into_iter().chain(entry.resting))
            .map(|due| WakeHint::In(due.saturating_duration_since(now)))
            .fold(WakeHint::Idle, WakeHint::soonest)
    }

    fn next_wake(&self) -> WakeHint {
        let now = Instant::now();
        self.hosted
            .values()
            .filter(|entry| !entry.painting && !entry.awaiting && entry.drawable)
            .filter_map(|entry| entry.ui.next_wake())
            .map(|at| WakeHint::In(at.saturating_duration_since(now)))
            .fold(WakeHint::Idle, WakeHint::soonest)
    }

    fn in_flight(&self) -> bool {
        self.hosted.values().any(|entry| entry.painting)
    }

    fn collect_painted(&mut self) -> Vec<(K, Option<RenderOutput>)> {
        if let Some(painter) = self.painter.as_ref() {
            for done in painter.take_painted() {
                self.took(done);
            }
        }
        std::mem::take(&mut self.finished)
    }

    fn took(&mut self, done: PaintResult<K, W>) {
        let PaintResult {
            key,
            mut surface,
            recorded,
            again,
            settling,
        } = done;
        let Some(entry) = self.hosted.get_mut(&key) else {
            return;
        };
        entry.painting = false;
        entry.settling = settling;
        if again {
            entry.ui.request_redraw();
        }

        let output = recorded.and_then(|recorded| {
            if recorded.is_presentable() {
                entry.resting = Some(Instant::now() + TRIM_AFTER);
                if let Some(presenting) = entry.presenting.clone() {
                    presenting(&mut entry.state);
                }
            }
            match surface.present(recorded) {
                Ok(output) => Some(output),
                Err(error) => {
                    tracing::error!("failed to present frame: {error}");
                    None
                }
            }
        });
        entry.attach(surface);
        self.finished.push((key, output));
    }

    fn spawn_tasks(&mut self) {
        let Self { hosted, runner, .. } = self;
        for (key, entry) in hosted.iter_mut() {
            entry.tasks.apply(entry.ui.take_jobs(), runner, key);
        }
    }

    fn commit_resources(&mut self) -> ResourceFrame {
        let Self {
            hosted,
            painter,
            resources,
            ..
        } = self;
        let Some(painter) = painter.as_ref() else {
            return ResourceFrame::default();
        };
        resources.shown(hosted.values().flat_map(|entry| entry.ui.shown_images()));
        if !resources.wants_commit() {
            return ResourceFrame::default();
        }
        let frame = resources.commit(painter.uploads());
        for entry in hosted.values_mut() {
            entry
                .ui
                .images_arrived(&frame.settled, !frame.updated.is_empty() || frame.repainted);
            for texture in &frame.updated {
                entry.texture_updated(*texture);
            }
            for (key, pulse) in &frame.pulsed {
                if entry.ui.shows_image(key) {
                    Pulse::join(&mut entry.pulses, pulse.clone());
                }
            }
        }
        frame
    }

    fn follow_viewports(&mut self) {
        let Self {
            hosted,
            viewports,
            painter,
            wake,
            ..
        } = self;
        let Some(painter) = painter.as_ref() else {
            return;
        };
        if viewports.is_none() {
            if !any_offscreen_live() {
                return;
            }
            match Viewports::start(painter.shared_renderer(), wake.clone()) {
                Ok(started) => *viewports = Some(started),
                Err(error) => {
                    tracing::error!("failed to spawn viewport thread: {error}");
                    return;
                }
            }
        }
        let Some(viewports) = viewports.as_mut() else {
            return;
        };
        if let Some(frame) = viewports.take_drawn()
            && (!frame.updated.is_empty() || frame.appeared)
        {
            for entry in hosted.values_mut() {
                for texture in &frame.updated {
                    entry.texture_updated(*texture);
                }
                if frame.appeared {
                    entry.ui.request_frame();
                }
            }
        }
        let presentable = hosted
            .values()
            .any(|entry| entry.drawable && !entry.awaiting && !entry.painting);
        viewports.request_pass(presentable);
    }

    fn draw_dirty(&mut self, pace: Duration, mut drawing: impl FnMut(Drawing<'_, S>)) -> bool {
        let Self {
            hosted,
            painter,
            resources,
            ..
        } = self;
        let Some(painter) = painter.as_mut() else {
            return false;
        };
        let mut animating = false;
        for (key, entry) in hosted.iter_mut() {
            if !entry.drawable || (!entry.painting && entry.surface.is_none()) {
                continue;
            }
            let presentable = !entry.awaiting && !entry.painting;
            let ahead = !entry.built && !entry.animating;
            if entry.ui.needs_render() && (presentable || ahead) {
                let now = Instant::now();
                let shown = match presentable {
                    true => entry.clock.presenting(now, pace),
                    false => entry.clock.ahead(now, pace),
                };
                let Hosted {
                    ui, scene, state, ..
                } = entry;
                resources.follow_theme(ui.theme());
                ui.render(
                    Rendering::new(scene)
                        .with_time(now)
                        .shown_at(shown)
                        .resolving(resources),
                );
                drawing(Drawing { ui, state });
                entry.ui.take_pulses(&mut entry.pulses);
                entry.animating = entry.ui.needs_render();
                animating |= entry.animating;
                entry.send(key, painter);
            } else if presentable && entry.scene.has_changes() {
                entry.send(key, painter);
            }
            if presentable && std::mem::take(&mut entry.settling) {
                entry.built = true;
            }
            if !presentable || !entry.built {
                continue;
            }
            let Some(surface) = entry.surface.take() else {
                continue;
            };
            match painter.present(key, surface) {
                Ok(()) => {
                    entry.clock.commit_ahead();
                    entry.built = false;
                    entry.painting = true;
                    entry.awaiting = true;
                    for pulse in entry.pulses.drain(..) {
                        Pulse::join(&mut entry.pulsing, pulse);
                    }
                    if let Some(sealing) = entry.sealing.clone() {
                        sealing(&mut entry.state);
                    }
                }
                Err(refused) => {
                    entry.built = false;
                    entry.resend = true;
                    entry.attach(*refused);
                    entry.ui.request_redraw();
                }
            }
        }
        animating
    }

    pub fn deliver(&mut self, key: &K, inbound: RuntimeMessage) {
        if let Some(entry) = self.hosted.get_mut(key) {
            entry.ui.receive(inbound);
        }
    }

    pub fn shutdown(&mut self) {
        self.viewports = None;
        let keys: Vec<K> = self.hosted.keys().cloned().collect();
        for key in keys {
            self.remove(&key);
        }
        if self.painter.is_some()
            && let Some(compute) = SharedGpu::compute(Duration::ZERO)
            && let Err(error) = compute.wait_idle()
        {
            tracing::warn!("image worker panicked during shutdown: {error}");
        }
        self.painter = None;
    }
}

fn share_gpu(renderer: &Renderer) {
    #[cfg(unix)]
    SharedGpu::from_gpu(renderer.gpu()).publish();
    if SharedGpu::compute(Duration::ZERO).is_some() {
        return;
    }
    match renderer.gpu().compute() {
        Ok(compute) => SharedGpu::publish_compute(compute),
        Err(error) => tracing::warn!(%error, "GPU compute unavailable"),
    }
}

fn wake_hint(animating: bool, resources: &ResourceFrame) -> WakeHint {
    let mut hint = WakeHint::Idle;
    if animating || resources.loading {
        hint = hint.soonest(WakeHint::In(LOOK_AGAIN));
    }
    if let Some(due) = resources.due {
        hint = hint.soonest(WakeHint::In(due));
    }
    hint
}

impl<K: std::fmt::Debug, S, W> std::fmt::Debug for Embedder<K, S, W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Embedder")
            .field("surfaces", &self.hosted.len())
            .field("has_renderer", &self.painter.is_some())
            .finish_non_exhaustive()
    }
}
