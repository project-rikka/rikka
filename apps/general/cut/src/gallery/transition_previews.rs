use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel};
use std::time::{Duration, Instant};

use cut_project::TransitionKind;
use rikka_image_edit::{Imaging, Picture};
use rikka_picture::SharedGpu;
use rikka_sync::Mutex;
use rikka_ui::{ComputedImage, ImageSource};
use rikka_worker::{Stop, Worker};

use super::cards::{Cards, LOOP, POSTER};

const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 30);
const THREAD_NAME: &str = "rikka-cut-gallery";

#[derive(Clone, Default)]
pub(crate) struct TransitionPreviews(Rc<Inner>);

struct Inner {
    posters: Vec<ComputedImage>,
    strips: Vec<ComputedImage>,
    playing: Cell<Option<TransitionKind>>,
    jobs: RefCell<Option<Sender<Job>>>,
    worker: Arc<Mutex<Option<Worker<Jobs>>>>,
}

enum Job {
    Play(Option<TransitionKind>),
    Stop,
}

struct Jobs(Sender<Job>);

impl Stop for Jobs {
    fn stop(&self) {
        if self.0.send(Job::Stop).is_err() {
            tracing::trace!("transition gallery already stopped");
        }
    }
}

impl Default for Inner {
    fn default() -> Inner {
        let slots = || TransitionKind::ALL.map(|_| ComputedImage::new()).to_vec();
        Inner {
            posters: slots(),
            strips: slots(),
            playing: Cell::new(None),
            jobs: RefCell::new(None),
            worker: Arc::new(Mutex::new(None)),
        }
    }
}

impl TransitionPreviews {
    pub(crate) fn same(&self, other: &TransitionPreviews) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn picture(&self, kind: TransitionKind) -> Option<ImageSource> {
        self.start();
        let at = kind.index();
        let strip = &self.0.strips[at];
        let picture = match self.0.playing.get() == Some(kind) && strip.size().is_some() {
            true => strip,
            false => &self.0.posters[at],
        };
        picture.size()?;
        Some(ImageSource::Computed(picture.clone()))
    }

    pub(crate) fn play(&self, kind: Option<TransitionKind>) {
        if self.0.playing.get() == kind {
            return;
        }
        self.0.playing.set(kind);
        self.start();
        if let Some(jobs) = self.0.jobs.borrow().as_ref()
            && jobs.send(Job::Play(kind)).is_err()
        {
            tracing::debug!("transition gallery stopped");
        }
    }

    fn start(&self) {
        let mut jobs = self.0.jobs.borrow_mut();
        if jobs.is_some() {
            return;
        }
        let (sender, received) = channel();
        let stopping = Jobs(sender.clone());
        *jobs = Some(sender);
        let posters = self.0.posters.clone();
        let strips = self.0.strips.clone();
        let slot = Arc::downgrade(&self.0.worker);
        SharedGpu::on_compute(move |compute| {
            let Some(slot) = slot.upgrade() else {
                return;
            };
            let imaging = Imaging::shared(&compute);
            let spawned = Worker::spawn(THREAD_NAME, stopping, move || {
                Ok::<_, Infallible>(move || work(&imaging, &posters, &strips, &received))
            });
            match spawned {
                Ok(worker) => *slot.lock() = Some(worker),
                Err(error) => tracing::warn!(%error, "failed to render transition gallery"),
            }
        });
    }
}

impl std::fmt::Debug for TransitionPreviews {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reels")
            .field("playing", &self.0.playing.get())
            .finish_non_exhaustive()
    }
}

fn work(
    imaging: &Imaging,
    posters: &[ComputedImage],
    strips: &[ComputedImage],
    jobs: &Receiver<Job>,
) {
    let cards = match Cards::new(imaging) {
        Ok(cards) => cards,
        Err(error) => {
            tracing::warn!(%error, "failed to render transition gallery");
            return;
        }
    };
    let mut playing: Option<TransitionKind> = None;
    for (kind, poster) in TransitionKind::ALL.into_iter().zip(posters) {
        match jobs.try_recv() {
            Ok(Job::Play(requested)) => playing = requested,
            Ok(Job::Stop) | Err(TryRecvError::Disconnected) => return,
            Err(TryRecvError::Empty) => {}
        }
        match cards
            .frame(kind, POSTER)
            .and_then(|picture| picture.shown())
        {
            Ok(frame) => poster.show(frame),
            Err(error) => tracing::warn!(%error, "failed to render transition thumbnail"),
        }
    }
    let mut reel: Vec<Picture> = Vec::new();
    let mut step = 0;
    let mut due = Instant::now();
    loop {
        let target = match playing {
            None => jobs.recv().map_err(|_| RecvTimeoutError::Disconnected),
            Some(_) => jobs.recv_timeout(due.saturating_duration_since(Instant::now())),
        };
        match target {
            Ok(Job::Stop) | Err(RecvTimeoutError::Disconnected) => return,
            Ok(Job::Play(kind)) => {
                if kind != playing {
                    reel.clear();
                }
                playing = kind;
                step = 0;
                due = Instant::now();
                continue;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        let Some(kind) = playing else {
            continue;
        };
        let picture = match reel.get(step) {
            Some(picture) => Ok(picture.clone()),
            None => cards
                .frame(kind, step as i64)
                .inspect(|made| reel.push(made.clone())),
        };
        match picture.and_then(|picture| picture.shown()) {
            Ok(frame) => strips[kind.index()].show(frame),
            Err(error) => {
                tracing::warn!(%error, "failed to render transition preview");
                playing = None;
            }
        }
        step = (step + 1) % LOOP as usize;
        due += FRAME;
        let now = Instant::now();
        if due < now {
            due = now;
        }
    }
}
