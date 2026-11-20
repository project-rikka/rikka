use rikka_sync::{Condvar, Mutex, MutexGuard};
use std::collections::BTreeSet;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cut_project::MediaId;
use rikka_gpu::ComputedFrame;
use rikka_image_edit::{Imaging, Picture};
use rikka_worker::{Stop, Worker};

use crate::compositor::Compositor;
use crate::error::{Error, Result};
use crate::request::Request;

const PRESENT_LATENCY: Duration = Duration::from_micros(16_700);
const PAUSED_FOOTAGE_TIMEOUT: Duration = Duration::from_millis(80);
const LATENCY_SMOOTHING: f64 = 0.2;
const DEADLINE_MARGIN: Duration = Duration::from_micros(500);
const IDLE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Debug)]
pub struct RenderedFrame {
    pub picture: ComputedFrame,
    pub frame: i64,
    pub exact: bool,
    pub offline: Arc<BTreeSet<MediaId>>,
}

#[derive(Default)]
struct RequestState {
    request: Option<Arc<Request>>,
    requested: u64,
    woken: bool,
    quit: bool,
    completed: u64,
    exact: bool,
    awaiting_footage: bool,
}

#[derive(Default)]
struct Shared {
    state: Mutex<RequestState>,
    signal: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, RequestState> {
        self.state.lock()
    }

    fn wake(&self) {
        let mut state = self.lock();
        if !state.awaiting_footage {
            return;
        }
        state.woken = true;
        drop(state);
        self.signal.notify_one();
    }

    fn await_footage(&self) {
        self.lock().awaiting_footage = true;
    }

    fn next(
        &self,
        handled: u64,
        deadline: Option<Instant>,
        idle: &dyn Fn(),
    ) -> Option<(Arc<Request>, u64)> {
        let mut state = self.lock();
        let mut idled = false;
        loop {
            if state.quit {
                return None;
            }
            let due = deadline.is_some_and(|deadline| Instant::now() >= deadline);
            if let Some(request) = state.request.clone()
                && (state.requested != handled || state.woken || due)
            {
                state.woken = false;
                return Some((request, state.requested));
            }
            state = match deadline {
                Some(deadline) => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    self.signal.wait_timeout(state, left).0
                }
                None if idled => self.signal.wait(state),
                None => {
                    let (guard, waited) = self.signal.wait_timeout(state, IDLE_TIMEOUT);
                    match waited.timed_out() {
                        true => {
                            drop(guard);
                            idle();
                            idled = true;
                            self.lock()
                        }
                        false => guard,
                    }
                }
            };
        }
    }

    fn complete(&self, requested: u64, exact: bool) {
        let mut state = self.lock();
        state.completed = requested;
        state.exact = exact;
        state.awaiting_footage = !exact;
    }
}

impl Stop for Shared {
    fn stop(&self) {
        self.lock().quit = true;
        self.signal.notify_one();
    }
}

pub struct Renderer {
    worker: Worker<Arc<Shared>>,
}

impl std::fmt::Debug for Renderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.shared().lock();
        f.debug_struct("Renderer")
            .field("requested", &state.requested)
            .field("completed", &state.completed)
            .finish_non_exhaustive()
    }
}

impl Renderer {
    pub fn start(
        imaging: Imaging,
        publish: impl Fn(&RenderedFrame) + Send + Sync + 'static,
    ) -> Result<Renderer> {
        let shared = Arc::new(Shared::default());
        let thread = Arc::clone(&shared);
        let worker = Worker::spawn("rikka-cut-render", shared, move || {
            let compositor = Compositor::new(imaging);
            Ok::<_, Infallible>(move || run(&thread, compositor, &publish))
        })?;
        Ok(Renderer { worker })
    }

    pub fn submit(&self, request: Request) {
        let mut state = self.shared().lock();
        if state
            .request
            .as_deref()
            .is_some_and(|pending| *pending == request)
        {
            return;
        }
        state.request = Some(Arc::new(request));
        state.requested += 1;
        drop(state);
        self.shared().signal.notify_one();
    }

    #[must_use]
    pub fn is_current(&self) -> bool {
        let state = self.shared().lock();
        state.completed == state.requested && state.exact
    }

    fn shared(&self) -> &Shared {
        self.worker.handle()
    }
}

#[derive(Default)]
struct Latency(Duration);

impl Latency {
    fn lead(&self) -> Duration {
        self.0 + PRESENT_LATENCY
    }

    fn measure(&mut self, taken: Duration) {
        self.0 = match self.0.is_zero() {
            true => taken,
            false => self.0.mul_f64(1.0 - LATENCY_SMOOTHING) + taken.mul_f64(LATENCY_SMOOTHING),
        };
    }
}

struct Published {
    picture: Picture,
    offline: Arc<BTreeSet<MediaId>>,
}

fn run(shared: &Arc<Shared>, mut compositor: Compositor, publish: &dyn Fn(&RenderedFrame)) {
    let waker = Arc::downgrade(shared);
    compositor.on_ready(move || {
        if let Some(shared) = waker.upgrade() {
            shared.wake();
        }
    });
    let mut handled = 0;
    let mut deadline = None;
    let mut waiting: Option<(i64, Instant)> = None;
    let mut latency = Latency::default();
    let mut published: Option<Published> = None;
    let mut warned = false;
    while let Some((request, requested)) = shared.next(handled, deadline, &|| compositor.idle()) {
        handled = requested;
        let started = Instant::now();
        let frame = request.due(started + latency.lead());
        let frame_deadline = next_deadline(&request, frame, latency.lead());
        shared.await_footage();
        let inputs = compositor.collect_inputs(&request, frame);
        if inputs.is_ready() {
            waiting = None;
        } else {
            let waiting_since = match waiting {
                Some((pending, waiting_since)) if pending == frame => waiting_since,
                _ => started,
            };
            waiting = Some((frame, waiting_since));
            let footage_timeout = waiting_since + footage_timeout(&request);
            if started < footage_timeout {
                deadline =
                    Some(frame_deadline.map_or(footage_timeout, |next| next.min(footage_timeout)));
                continue;
            }
        }
        match compositor.compose(&request, frame, &inputs) {
            Ok(composite) => {
                let offline = Arc::new(composite.offline);
                let changed = published.as_ref().is_none_or(|published| {
                    published.picture != composite.picture || published.offline != offline
                });
                if changed {
                    match composite.picture.shown() {
                        Ok(picture) => {
                            publish(&RenderedFrame {
                                picture,
                                frame,
                                exact: composite.exact,
                                offline: Arc::clone(&offline),
                            });
                            published = Some(Published {
                                picture: composite.picture,
                                offline,
                            });
                        }
                        Err(error) => warn_once(&mut warned, &error.into()),
                    }
                }
                shared.complete(requested, composite.exact);
                warned = false;
            }
            Err(error) => warn_once(&mut warned, &error),
        }
        latency.measure(started.elapsed());
        deadline = next_deadline(&request, frame, latency.lead());
    }
}

fn footage_timeout(request: &Request) -> Duration {
    match request.clock() {
        Some(_) => Duration::from_secs_f64(0.5 / request.per_second().max(1.0)),
        None => PAUSED_FOOTAGE_TIMEOUT,
    }
}

fn next_deadline(request: &Request, frame: i64, lead: Duration) -> Option<Instant> {
    let playing = request.clock()?;
    let boundary = match playing.speed > 0.0 {
        true => frame as f64 + 1.0,
        false => frame as f64,
    };
    let reached = playing.reaching(boundary, request.per_second())?;
    Some(reached.checked_sub(lead).unwrap_or(reached) + DEADLINE_MARGIN)
}

fn warn_once(warned: &mut bool, error: &Error) {
    if !*warned {
        tracing::warn!(%error, "failed to render preview");
        *warned = true;
    }
}
