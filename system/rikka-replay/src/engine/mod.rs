mod coder;
mod gpu;
mod subject;
mod target;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use rikka_capture::Screen;
use rikka_worker::{CommandReceiver, CommandSender, Worker, command_channel};

use crate::clock;
use crate::engine::gpu::Gpu;
use crate::engine::subject::{Health, Subject};
use crate::error::{Error, Result};
use crate::options::CaptureOptions;
use crate::store::SharedStore;

pub(crate) use target::{WindowInfo, display_subject, window_subject};

const THREAD: &str = "rikka-replay";
const TRIM_INTERVAL: Duration = Duration::from_secs(1);
const RETRY_BACKOFF: Duration = Duration::from_secs(10);

pub(crate) type OnChange = Box<dyn Fn() + Send>;

enum Command {
    SetCapture(CaptureOptions),
    Windows(Vec<WindowInfo>),
    Stop,
}

pub(crate) struct Engine {
    worker: Worker<CommandSender<Command>>,
}

impl Engine {
    pub(crate) fn start(
        capture: CaptureOptions,
        window_info: Vec<WindowInfo>,
        store: SharedStore,
        on_change: OnChange,
    ) -> Result<Engine> {
        let (commands, receiver) = command_channel(|| Command::Stop).map_err(Error::Workers)?;
        let worker = Worker::spawn(THREAD, commands, move || {
            let screen = Screen::connect()?;
            Ok::<_, rikka_capture::Error>(move || {
                Recorder {
                    subjects: Vec::new(),
                    gpu: Gpu::default(),
                    screen,
                    commands: receiver,
                    capture,
                    window_info,
                    store,
                    on_change,
                    backoff: HashMap::new(),
                    last_trim: Instant::now(),
                    seen_changes: None,
                }
                .run();
            })
        })
        .map_err(|error| match error {
            rikka_worker::Error::Spawn(error) => Error::Workers(error),
            rikka_worker::Error::Startup(error) => Error::Screen(error),
            rikka_worker::Error::Exited => Error::Workers(std::io::Error::other(
                "the replay's worker stopped before it began",
            )),
        })?;
        Ok(Engine { worker })
    }

    pub(crate) fn set_capture(&self, capture: CaptureOptions) {
        self.send(Command::SetCapture(capture));
    }

    pub(crate) fn windows(&self, window_info: Vec<WindowInfo>) {
        self.send(Command::Windows(window_info));
    }

    fn send(&self, command: Command) {
        if let Err(error) = self.worker.handle().send(command) {
            tracing::warn!("the replay worker is gone: {error}");
        }
    }
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine").finish_non_exhaustive()
    }
}

struct Recorder {
    subjects: Vec<Subject>,
    gpu: Gpu,
    screen: Screen,
    commands: CommandReceiver<Command>,
    capture: CaptureOptions,
    window_info: Vec<WindowInfo>,
    store: SharedStore,
    on_change: OnChange,
    backoff: HashMap<String, Instant>,
    last_trim: Instant,
    seen_changes: Option<u64>,
}

impl Recorder {
    fn run(mut self) {
        loop {
            let commands: Vec<Command> = self.commands.drain().collect();
            for command in commands {
                match command {
                    Command::SetCapture(capture) => self.set_capture(capture),
                    Command::Windows(window_info) => {
                        self.window_info = window_info;
                        self.seen_changes = None;
                    }
                    Command::Stop => return self.finish(),
                }
            }
            if self.is_stale() {
                self.reconcile();
            }
            let now = Instant::now();
            let mut added = false;
            for subject in &mut self.subjects {
                added |= subject.pump(
                    &mut self.screen,
                    &mut self.gpu,
                    &self.capture,
                    &self.store,
                    now,
                );
            }
            if added {
                (self.on_change)();
            }
            self.remove_finished();
            if let Err(error) = self.screen.flush() {
                tracing::error!("compositor connection lost: {error}");
                return self.finish();
            }
            let wait = self.deadline(now).saturating_duration_since(Instant::now());
            let commands = &self.commands;
            if let Err(error) = self
                .screen
                .dispatch_with(|fd| commands.wait_readable(fd, wait))
            {
                tracing::error!("compositor connection lost: {error}");
                return self.finish();
            }
            for subject in &mut self.subjects {
                subject.collect_frames(&self.store);
            }
            self.remove_finished();
            if self.last_trim.elapsed() >= TRIM_INTERVAL {
                self.store.lock().trim(clock::now_us());
                self.last_trim = Instant::now();
            }
        }
    }

    fn deadline(&self, now: Instant) -> Instant {
        let interval = Duration::from_secs_f64(1.0 / f64::from(self.capture.fps));
        self.subjects
            .iter()
            .filter_map(|subject| subject.wake_at(interval, now))
            .chain(self.backoff.values().copied())
            .chain(std::iter::once(self.last_trim + TRIM_INTERVAL))
            .min()
            .unwrap_or(now + TRIM_INTERVAL)
    }

    fn set_capture(&mut self, capture: CaptureOptions) {
        if capture == self.capture {
            return;
        }
        self.capture = capture;
        for subject in &mut self.subjects {
            subject.end(&self.store);
        }
        self.subjects.clear();
        self.backoff.clear();
        self.seen_changes = None;
        (self.on_change)();
    }

    fn is_stale(&self) -> bool {
        let now = Instant::now();
        self.seen_changes != Some(self.screen.changes())
            || self.backoff.values().any(|until| now >= *until)
    }

    fn reconcile(&mut self) {
        self.seen_changes = Some(self.screen.changes());
        let targets = target::targets(
            &self.capture,
            &self.screen.outputs(),
            &self.screen.windows(),
            &self.window_info,
        );
        let mut changed = false;
        let store = &self.store;
        self.subjects.retain_mut(|subject| {
            let still_targeted = targets
                .iter()
                .any(|target| target.subject == subject.subject());
            if !still_targeted {
                subject.end(store);
                changed = true;
            }
            still_targeted
        });
        let now = Instant::now();
        self.backoff.retain(|_, until| now < *until);
        for target in targets {
            if let Some(subject) = self
                .subjects
                .iter_mut()
                .find(|subject| subject.subject() == target.subject)
            {
                changed |= subject.update_kind(target, &self.store);
                continue;
            }
            if self.backoff.contains_key(&target.subject) {
                continue;
            }
            let named = target.subject.clone();
            match Subject::start(&mut self.screen, target, self.capture.cursor) {
                Ok(subject) => self.subjects.push(subject),
                Err(reason) => {
                    tracing::warn!("cannot capture {named}: {reason}");
                    self.backoff.insert(named, now + RETRY_BACKOFF);
                }
            }
        }
        if changed {
            (self.on_change)();
        }
    }

    fn remove_finished(&mut self) {
        let before = self.subjects.len();
        let now = Instant::now();
        let store = &self.store;
        let backoff = &mut self.backoff;
        self.subjects.retain_mut(|subject| {
            match subject.health() {
                Health::Alive => return true,
                Health::Ended => {
                    tracing::info!("{} closed", subject.subject());
                    backoff.insert(subject.subject().to_owned(), now + RETRY_BACKOFF);
                }
                Health::Broken(reason) => {
                    tracing::warn!("capture of {} failed: {reason}", subject.subject());
                    backoff.insert(subject.subject().to_owned(), now + RETRY_BACKOFF);
                }
            }
            subject.end(store);
            false
        });
        if self.subjects.len() != before {
            (self.on_change)();
        }
    }

    fn finish(&mut self) {
        for subject in &mut self.subjects {
            subject.end(&self.store);
        }
        self.subjects.clear();
    }
}
