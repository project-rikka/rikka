use std::hash::Hash;
use std::time::{Duration, Instant};

use rikka_renderer::{ExternalImage, ExternalTarget, Renderer, Result, Scene};
use rustc_hash::FxHashMap;

use rikka_picture::{Pulse, RefreshRate, Tick};
use rikka_ui::{Rendering, RuntimeMessage, Ui};

use crate::panes::Panes;
use crate::resources::Resources;
use crate::runner::{TaskRunner, TaskSink};
use crate::tasks::Tasks;
use crate::video::VideoPlayback;

const PREFETCH_BUDGET: Duration = Duration::from_millis(1);

pub struct Composed {
    pub ui: Ui,
    scene: Scene,
    target: ExternalTarget,
    panes: Panes,
    pulses: Vec<Pulse>,
    tasks: Tasks,
}

impl Composed {
    fn beat(&mut self, at: Instant) {
        let tick = Tick {
            at,
            period: RefreshRate::published_period(),
        };
        for pulse in self.pulses.drain(..) {
            pulse.beat(tick);
        }
    }
}

impl std::fmt::Debug for Composed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Composed")
            .field("tasks", &self.tasks.len())
            .finish_non_exhaustive()
    }
}

pub struct Composer<K> {
    renderer: Renderer,
    resources: Resources,
    runner: TaskRunner<K>,
    composed: FxHashMap<K, Composed>,
}

impl<K> Composer<K>
where
    K: Eq + Hash + Clone + Send + 'static,
{
    pub fn new(renderer: Renderer, sink: impl TaskSink<K>) -> Self {
        Self {
            renderer,
            resources: Resources::new(),
            runner: TaskRunner::new(sink),
            composed: FxHashMap::default(),
        }
    }

    pub fn plays_video(&mut self, videos: Box<dyn VideoPlayback>) {
        self.resources.plays_video(videos);
    }

    pub fn insert(&mut self, key: K, ui: impl Into<Ui>) -> &mut Composed {
        let target = self.renderer.create_external_target();
        self.composed
            .entry(key)
            .insert_entry(Composed {
                ui: ui.into(),
                scene: Scene::new(),
                target,
                panes: Panes::default(),
                pulses: Vec::new(),
                tasks: Tasks::default(),
            })
            .into_mut()
    }

    pub fn remove(&mut self, key: &K) -> Option<Ui> {
        self.composed.remove(key).map(|composed| composed.ui)
    }

    pub fn get(&self, key: &K) -> Option<&Composed> {
        self.composed.get(key)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut Composed> {
        self.composed.get_mut(key)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut Composed)> {
        self.composed.iter_mut()
    }

    pub fn deliver(&mut self, key: &K, inbound: RuntimeMessage) {
        if let Some(composed) = self.composed.get_mut(key) {
            composed.ui.receive(inbound);
        }
    }

    pub unsafe fn draw(&mut self, key: &K, image: ExternalImage) -> Result<()> {
        self.spawn_tasks();
        self.commit_resources();
        let Self {
            renderer,
            resources,
            composed,
            ..
        } = self;
        let Some(composed) = composed.get_mut(key) else {
            return Ok(());
        };
        let now = Instant::now();
        if composed.ui.needs_render() {
            resources.follow_theme(composed.ui.theme());
            composed.ui.render(
                Rendering::new(&mut composed.scene)
                    .with_time(now)
                    .resolving(resources),
            );
            composed.ui.take_pulses(&mut composed.pulses);
        }
        composed.panes.follow(
            &mut composed.scene,
            composed.ui.glass(),
            composed.ui.theme().translucency(),
            composed.ui.scale_factor(),
        );
        let drawn = renderer.draw(&mut composed.scene, unsafe { composed.target.onto(image) });
        composed.beat(Instant::now());
        if composed.ui.wants_prefetch() {
            composed
                .ui
                .prefetch(&mut composed.scene, Instant::now() + PREFETCH_BUDGET);
        }
        drawn.map(drop)
    }

    fn spawn_tasks(&mut self) {
        let Self {
            composed, runner, ..
        } = self;
        for (key, entry) in composed.iter_mut() {
            entry.tasks.apply(entry.ui.take_jobs(), runner, key);
        }
    }

    fn commit_resources(&mut self) {
        let Self {
            renderer,
            resources,
            composed,
            ..
        } = self;
        resources.shown(composed.values().flat_map(|entry| entry.ui.shown_images()));
        if !resources.wants_commit() {
            return;
        }
        let frame = resources.commit(&renderer.uploads());
        for entry in composed.values_mut() {
            entry
                .ui
                .images_arrived(&frame.settled, !frame.updated.is_empty() || frame.repainted);
            for texture in &frame.updated {
                entry.scene.notify_texture_updated(*texture);
            }
            for (key, pulse) in &frame.pulsed {
                if entry.ui.shows_image(key) {
                    Pulse::join(&mut entry.pulses, pulse.clone());
                }
            }
        }
    }
}

impl<K> std::fmt::Debug for Composer<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Composer")
            .field("composed", &self.composed.len())
            .finish_non_exhaustive()
    }
}
