use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::time::{Duration, Instant};

use animate_cast::FigureId;
use animate_shader::{Shaders, SharedCompiler};
use animate_stage::{Sitting, StageError, still_viewport};
use rikka_model::Instance;
use rikka_model_gpu::Prepared;
use rikka_renderer::Renderer;
use rikka_renderer_3d::OffscreenViewport;
use rikka_sync::Mutex;
use rikka_types::PixelSize;

use crate::delivery::{Delivery, Picture};
use crate::error::{BoothError, reason};
use crate::look::look;
use crate::pool::Queue;
use crate::setup::{Scope, Setup};
use crate::sitter::ModelKey;
use crate::work::{Message, Staged, Task};

const KEPT_SEATS: usize = 4;
const KEPT_VIEWPORTS: usize = 4;
const PREVIEW_RATE: u64 = 30;
const PREVIEW_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / PREVIEW_RATE);
const SETTLE_LIMIT: Duration = Duration::from_secs(10);
const APP_NAME: &str = "rikka-animate";

struct Seat {
    figure: FigureId,
    instance: Instance,
    used: u64,
}

struct Live {
    staged: Staged,
    started: Instant,
    due: Instant,
    shown: bool,
}

struct Held {
    staged: Staged,
    since: Instant,
}

enum Wait {
    Now,
    Until(Instant),
    Idle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pass {
    Still,
    Frame,
}

#[derive(Clone, Copy)]
struct Shooting {
    time: f32,
    pass: Pass,
    scope: Scope,
}

struct Drawn {
    picture: Picture,
    settled: bool,
}

enum Showing {
    Playing,
    Finished,
    Unseated,
}

pub(crate) struct Studio {
    renderer: Renderer,
    stage: animate_stage::Studio,
    queue: Queue<Task>,
    seats: HashMap<ModelKey, Seat>,
    parked: HashMap<ModelKey, Vec<Staged>>,
    viewports: Vec<(PixelSize, OffscreenViewport)>,
    stills: Vec<Staged>,
    lives: Vec<Live>,
    held: Option<Held>,
    roused: bool,
    uses: u64,
}

fn open_renderer() -> Result<Renderer, Arc<str>> {
    let gpu = rikka_gpu::Gpu::builder()
        .app_name(APP_NAME)
        .with_3d()
        .build()
        .map_err(|error| reason(&error))?;
    Renderer::new(&gpu).map_err(|error| reason(&error))
}

pub(crate) fn run(queue: Queue<Task>, rouse: Sender<Message>, inbox: Receiver<Message>) {
    match open_renderer() {
        Ok(renderer) => Studio::new(renderer, queue, rouse).run(inbox),
        Err(reason) => refuse(&inbox, &reason),
    }
}

fn refuse(inbox: &Receiver<Message>, reason: &Arc<str>) {
    tracing::error!("booth renderer is unavailable: {reason}");
    while let Ok(message) = inbox.recv() {
        match message {
            Message::Staged(staged) => staged
                .delivery
                .fail(BoothError::Unavailable(Arc::clone(reason))),
            Message::Seat { .. } | Message::Unseated { .. } | Message::Roused => {}
            Message::Stop => return,
        }
    }
}

fn refuse_seat(parked: Vec<Staged>, reason: &Arc<str>) {
    for staged in parked {
        staged
            .delivery
            .fail(BoothError::Unseated(Arc::clone(reason)));
    }
}

fn viewport<'a>(
    viewports: &'a mut Vec<(PixelSize, OffscreenViewport)>,
    renderer: &mut Renderer,
    size: PixelSize,
) -> Result<&'a mut OffscreenViewport, StageError> {
    match viewports.iter().position(|(known, _)| *known == size) {
        Some(at) => viewports[..=at].rotate_right(1),
        None => {
            let opened = still_viewport(renderer, size)?;
            viewports.insert(0, (size, opened));
            viewports.truncate(KEPT_VIEWPORTS);
        }
    }
    Ok(&mut viewports[0].1)
}

impl Studio {
    fn new(renderer: Renderer, queue: Queue<Task>, rouse: Sender<Message>) -> Self {
        let mut stage = animate_stage::Studio::new(
            Arc::new(Mutex::new(Shaders::new())),
            SharedCompiler::default(),
            Arc::new(look()),
        );
        stage.notify_with(Arc::new(move || {
            let _ = rouse.send(Message::Roused);
        }));
        Self {
            renderer,
            stage,
            queue,
            seats: HashMap::new(),
            parked: HashMap::new(),
            viewports: Vec::new(),
            stills: Vec::new(),
            lives: Vec::new(),
            held: None,
            roused: false,
            uses: 0,
        }
    }

    fn run(mut self, inbox: Receiver<Message>) {
        loop {
            let now = Instant::now();
            let message = match self.wait(now) {
                Wait::Now => match inbox.try_recv() {
                    Ok(message) => Some(message),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => return,
                },
                Wait::Until(deadline) => {
                    match inbox.recv_timeout(deadline.saturating_duration_since(now)) {
                        Ok(message) => Some(message),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                Wait::Idle => match inbox.recv() {
                    Ok(message) => Some(message),
                    Err(_) => return,
                },
            };
            if let Some(message) = message {
                if !self.handle(message) {
                    return;
                }
                while let Ok(message) = inbox.try_recv() {
                    if !self.handle(message) {
                        return;
                    }
                }
            }
            self.tick(Instant::now());
        }
    }

    fn wait(&self, now: Instant) -> Wait {
        let live = self.lives.iter().map(|live| live.due).min();
        match (&self.held, live) {
            (Some(_), _) if self.roused => Wait::Now,
            (Some(held), live) => {
                let settled = held.since + SETTLE_LIMIT;
                Wait::Until(live.map_or(settled, |due| due.min(settled)))
            }
            (None, _) if !self.stills.is_empty() => Wait::Now,
            (None, Some(due)) if due <= now => Wait::Now,
            (None, Some(due)) => Wait::Until(due),
            (None, None) => Wait::Idle,
        }
    }

    fn handle(&mut self, message: Message) -> bool {
        match message {
            Message::Staged(staged) => self.arrive(*staged),
            Message::Seat { key, prepared } => self.seat(key, *prepared),
            Message::Unseated { key, reason } => self.unseat(&key, &reason),
            Message::Roused => self.roused = true,
            Message::Stop => return false,
        }
        true
    }

    fn arrive(&mut self, staged: Staged) {
        if staged.delivery.is_cancelled() {
            return;
        }
        if !self.seats.contains_key(&staged.setup.key) {
            return self.park(staged);
        }
        match staged.delivery {
            Delivery::Still { .. } => self.stills.push(staged),
            Delivery::Live { .. } => {
                let now = Instant::now();
                self.lives.push(Live {
                    staged,
                    started: now,
                    due: now,
                    shown: false,
                });
            }
        }
    }

    fn park(&mut self, staged: Staged) {
        let key = staged.setup.key.clone();
        let source = staged.setup.source.clone();
        let parked = self.parked.entry(key.clone()).or_default();
        let requested = !parked.is_empty();
        parked.push(staged);
        if !requested {
            self.queue.push(Task::Prepare {
                key,
                source,
                uploads: self.renderer.uploads(),
            });
        }
    }

    fn seat(&mut self, key: ModelKey, prepared: Prepared) {
        let Some(parked) = self.parked.remove(&key) else {
            return;
        };
        let figure = FigureId::fresh();
        match self.stage.seat(&mut self.renderer, figure, prepared) {
            Ok(instance) => {
                self.uses += 1;
                self.seats.insert(
                    key,
                    Seat {
                        figure,
                        instance,
                        used: self.uses,
                    },
                );
                self.evict();
                for staged in parked {
                    self.arrive(staged);
                }
            }
            Err(error) => refuse_seat(parked, &reason(&error)),
        }
    }

    fn unseat(&mut self, key: &ModelKey, reason: &Arc<str>) {
        refuse_seat(self.parked.remove(key).unwrap_or_default(), reason);
    }

    fn evict(&mut self) {
        while self.seats.len() > KEPT_SEATS {
            let busy: Vec<ModelKey> = self
                .lives
                .iter()
                .map(|live| live.staged.setup.key.clone())
                .chain(self.held.iter().map(|held| held.staged.setup.key.clone()))
                .collect();
            let Some(oldest) = self
                .seats
                .iter()
                .filter(|(key, _)| !busy.contains(key))
                .min_by_key(|(_, seat)| seat.used)
                .map(|(key, _)| key.clone())
            else {
                return;
            };
            if let Some(seat) = self.seats.remove(&oldest) {
                self.stage.dismiss(seat.figure);
            }
        }
    }

    fn tick(&mut self, now: Instant) {
        self.lives
            .retain(|live| !live.staged.delivery.is_cancelled());
        let mut at = 0;
        while at < self.lives.len() {
            if self.lives[at].due > now {
                at += 1;
                continue;
            }
            match self.show(at, now) {
                Showing::Playing => at += 1,
                Showing::Finished => {
                    self.lives.remove(at);
                }
                Showing::Unseated => {
                    let live = self.lives.remove(at);
                    self.park(live.staged);
                }
            }
        }
        if let Some(held) = self.held.take() {
            match self.roused || now >= held.since + SETTLE_LIMIT {
                true => {
                    self.roused = false;
                    self.finish(held.staged, held.since);
                }
                false => self.held = Some(held),
            }
            return;
        }
        while let Some(staged) = self.stills.pop() {
            if !staged.delivery.is_cancelled() {
                self.finish(staged, now);
                return;
            }
        }
    }

    fn show(&mut self, at: usize, now: Instant) -> Showing {
        let live = &self.lives[at];
        let setup = Arc::clone(&live.staged.setup);
        let size = live.staged.size;
        let looping = setup.length > 0.0;
        let time = match looping {
            true => now.duration_since(live.started).as_secs_f32() % setup.length,
            false => setup.still,
        };
        let pass = match live.shown {
            true => Pass::Frame,
            false => Pass::Still,
        };
        let shooting = Shooting {
            time,
            pass,
            scope: Scope::Loop,
        };
        let drawn = self.draw(&setup, size, shooting);
        let live = &mut self.lives[at];
        match drawn {
            Ok(Some(Drawn { picture, settled })) => {
                if let Delivery::Live { feed, .. } = &live.staged.delivery {
                    feed.show(picture.width, picture.height, picture.rgba);
                }
                live.shown = true;
                live.due = (live.due + PREVIEW_INTERVAL).max(now);
                match looping || !settled {
                    true => Showing::Playing,
                    false => Showing::Finished,
                }
            }
            Ok(None) => Showing::Unseated,
            Err(error) => {
                tracing::warn!("booth preview failed: {}", reason(&error));
                Showing::Finished
            }
        }
    }

    fn finish(&mut self, staged: Staged, since: Instant) {
        let setup = Arc::clone(&staged.setup);
        let shooting = Shooting {
            time: setup.still,
            pass: Pass::Still,
            scope: Scope::Still,
        };
        match self.draw(&setup, staged.size, shooting) {
            Ok(Some(drawn)) if drawn.settled || since.elapsed() >= SETTLE_LIMIT => {
                if let Delivery::Still { done, .. } = staged.delivery {
                    done(Ok(drawn.picture));
                }
            }
            Ok(Some(_)) => self.held = Some(Held { staged, since }),
            Ok(None) => self.park(staged),
            Err(error) => staged.delivery.fail(error),
        }
    }

    fn draw(
        &mut self,
        setup: &Setup,
        size: PixelSize,
        shooting: Shooting,
    ) -> Result<Option<Drawn>, BoothError> {
        let Some(seat) = self.seats.get_mut(&setup.key) else {
            return Ok(None);
        };
        self.uses += 1;
        seat.used = self.uses;
        setup.pose(&mut seat.instance, shooting.time);
        let viewport = viewport(&mut self.viewports, &mut self.renderer, size)?;
        let shot = setup.shot(
            &seat.instance,
            shooting.time,
            viewport.aspect(),
            shooting.scope,
        );
        let pass = shooting.pass;
        let sitting = Sitting {
            figure: seat.figure,
            instance: &seat.instance,
            head: setup.head(),
        };
        match pass {
            Pass::Still => self
                .stage
                .still(&mut self.renderer, viewport, shot, sitting)?,
            Pass::Frame => self
                .stage
                .frame(&mut self.renderer, viewport, shot, sitting)?,
        }
        let rgba = viewport
            .read_pixels(&mut self.renderer)
            .map_err(StageError::from)?;
        Ok(Some(Drawn {
            picture: Picture {
                width: size.width,
                height: size.height,
                rgba,
            },
            settled: self.stage.is_settled(),
        }))
    }
}
