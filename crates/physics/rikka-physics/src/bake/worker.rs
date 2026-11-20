use std::sync::Arc;
use std::time::{Duration, Instant};

use rikka_gpu::Compute;
use rikka_gpu::Ticket;

use super::handle::Shared;
use crate::error::Result;
use crate::scene::{BakeOptions, Scene};
use crate::scene_source::{SceneSource, has_emitters};

const NOTIFY_INTERVAL: Duration = Duration::from_millis(100);

enum Command {
    SetSource {
        source: Arc<dyn SceneSource>,
        since: Option<u32>,
        compute: Compute,
    },
    Step {
        wanted: u32,
        playhead: u32,
    },
    Quit,
}

struct Baked {
    step: u32,
    kept: bool,
}

struct Worker<'a> {
    shared: &'a Shared,
    scene: Option<Scene>,
    in_flight: Option<Ticket>,
    paced: bool,
    notified_at: Instant,
}

pub(super) fn run(shared: &Shared) {
    let mut worker = Worker {
        shared,
        scene: None,
        in_flight: None,
        paced: true,
        notified_at: Instant::now(),
    };
    loop {
        match worker.next_command() {
            Command::Quit => return,
            Command::SetSource {
                source,
                since,
                compute,
            } => worker.set_source(source, since, &compute),
            Command::Step { wanted, playhead } => worker.step(wanted, playhead),
        }
    }
}

impl Worker<'_> {
    fn next_command(&mut self) -> Command {
        let mut state = self.shared.state();
        loop {
            if state.quit {
                return Command::Quit;
            }
            if let Some(compute) = state.compute.clone()
                && let Some(source) = state.source.clone()
                && let Some(since) = state.pending.take()
            {
                state.busy = true;
                if let Some(since) = since {
                    state.frontier = state.frontier.and_then(|frontier| {
                        if frontier < since {
                            Some(frontier)
                        } else {
                            since.checked_sub(1)
                        }
                    });
                    state.epoch += 1;
                }
                return Command::SetSource {
                    source,
                    since,
                    compute,
                };
            }
            if let Some(wanted) = state.wanted().filter(|_| self.scene.is_some()) {
                state.busy = true;
                return Command::Step {
                    wanted,
                    playhead: state.playhead,
                };
            }
            if state.busy {
                if let Some(in_flight) = self.in_flight.take() {
                    drop(state);
                    let finished = in_flight.wait();
                    state = self.shared.state();
                    if let Err(failure) = finished {
                        state.error = Some(failure.into());
                    }
                    continue;
                }
                state.busy = false;
                drop(state);
                self.notify();
                state = self.shared.state();
                continue;
            }
            state = self.shared.wait(state);
        }
    }

    fn set_source(&mut self, source: Arc<dyn SceneSource>, since: Option<u32>, compute: &Compute) {
        self.paced = !compute.is_async();
        let layout = self.scene.as_ref().map(|scene| Arc::clone(scene.layout()));
        let applied = self.apply(source, since, compute);
        let coverage = self.shared.cache().coverage();
        let mut state = self.shared.state();
        let layout_changed = match (&layout, &self.scene) {
            (Some(was), Some(scene)) => !Arc::ptr_eq(was, scene.layout()),
            (None, None) => false,
            _ => true,
        };
        if layout_changed || applied.is_err() {
            state.epoch += 1;
            state.frontier = None;
        }
        state.coverage = coverage;
        state.full = false;
        match applied {
            Ok(()) => state.halted = false,
            Err(failure) => {
                state.halted = true;
                state.error = Some(failure);
                self.scene = None;
            }
        }
        if self.scene.is_none() {
            state.frontier = None;
        }
        state.layout = self.scene.as_ref().map(|scene| Arc::clone(scene.layout()));
        drop(state);
        self.notify();
    }

    fn apply(
        &mut self,
        source: Arc<dyn SceneSource>,
        since: Option<u32>,
        compute: &Compute,
    ) -> Result<()> {
        if !has_emitters(source.as_ref()) {
            self.scene = None;
            self.shared.cache().clear();
            return Ok(());
        }
        let Some(scene) = self
            .scene
            .as_mut()
            .filter(|scene| scene.matches_layout(source.as_ref()))
        else {
            self.scene = None;
            self.shared.cache().clear();
            self.scene = Some(Scene::build(compute, source)?);
            return Ok(());
        };
        let Some(since) = since else {
            return scene.wake_figures(source);
        };
        let restored = {
            let mut cache = self.shared.cache();
            cache.invalidate_from(since);
            since
                .checked_sub(1)
                .and_then(|before| cache.checkpoint_before(before))
        };
        if scene.step().is_none_or(|at| at < since) {
            return scene.wake_figures(source);
        }
        match restored {
            Some((at, checkpoint)) => scene.restore(source, at, &checkpoint),
            None => scene.restart(source),
        }
    }

    fn step(&mut self, wanted: u32, playhead: u32) {
        let Some(scene) = self.scene.as_mut() else {
            return;
        };
        let shared = self.shared;
        let baked = bake(
            shared,
            scene,
            &mut self.in_flight,
            self.paced,
            wanted,
            playhead,
        );
        if baked.is_err() {
            self.scene = None;
            shared.cache().clear();
        }
        let coverage = shared.cache().coverage();
        let mut state = shared.state();
        state.coverage = coverage;
        let failed = match baked {
            Ok(baked) => {
                state.frontier = Some(
                    state
                        .frontier
                        .map_or(baked.step, |frontier| frontier.max(baked.step)),
                );
                if baked.step == wanted && !baked.kept && state.playhead == playhead {
                    state.full = true;
                }
                false
            }
            Err(failure) => {
                state.halted = true;
                state.error = Some(failure);
                state.layout = None;
                state.frontier = None;
                state.epoch += 1;
                true
            }
        };
        let idle = state.wanted().is_none();
        drop(state);
        if failed || idle || self.notified_at.elapsed() >= NOTIFY_INTERVAL {
            self.notify();
        }
    }

    fn notify(&mut self) {
        self.notified_at = Instant::now();
        (self.shared.notify)();
    }
}

fn bake(
    shared: &Shared,
    scene: &mut Scene,
    in_flight: &mut Option<Ticket>,
    paced: bool,
    wanted: u32,
    playhead: u32,
) -> Result<Baked> {
    if scene.step() == Some(wanted) {
        let (capture, ready) = scene.capture(in_flight.as_ref().filter(|_| paced))?;
        *in_flight = Some(ready);
        let kept = shared.cache().insert(wanted, Some(capture), None, playhead);
        return Ok(Baked { step: wanted, kept });
    }
    seek(shared, scene, wanted)?;
    let step = scene.next_step();
    let (capture, checkpoint) = {
        let cache = shared.cache();
        (!cache.contains(step), cache.wants_checkpoint(step))
    };
    let baked = scene.bake(BakeOptions {
        capture,
        checkpoint,
        wait_for: in_flight.as_ref().filter(|_| paced),
    })?;
    *in_flight = Some(baked.ready);
    let kept = shared
        .cache()
        .insert(baked.step, baked.capture, baked.checkpoint, playhead);
    Ok(Baked {
        step: baked.step,
        kept,
    })
}

fn seek(shared: &Shared, scene: &mut Scene, wanted: u32) -> Result<()> {
    let next = scene.next_step();
    if next == wanted {
        return Ok(());
    }
    let checkpoint = wanted
        .checked_sub(1)
        .and_then(|before| shared.cache().checkpoint_before(before));
    match checkpoint {
        Some((at, checkpoint)) if next > wanted || at >= next => {
            scene.restore(scene.source(), at, &checkpoint)
        }
        _ if next > wanted => scene.restart(scene.source()),
        _ => Ok(()),
    }
}
