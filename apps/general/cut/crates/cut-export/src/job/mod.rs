mod film;
mod still;
mod tape;

use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cut_project::Project;
use rikka_gpu::ComputedFrame;
use rikka_image_edit::Imaging;
use rikka_types::CancelToken;
use rikka_video::EncoderBackend;
use rikka_worker::{Stop, Worker};

use crate::error::{Error, Result};
use crate::output::OutputFile;
use crate::plan::Plan;
use crate::settings::{Audio, Still, Video};
use crate::span::Span;

const PROGRESS_EVERY: Duration = Duration::from_millis(100);
const KILO: u64 = 1000;

type Reporter = Arc<dyn Fn(Report) + Send + Sync>;

#[derive(Clone, Debug)]
enum Work {
    Video { video: Video, span: Span },
    Audio { audio: Audio, span: Span },
    Still { still: Still, frame: i64 },
}

#[derive(Clone, Debug)]
pub struct Order {
    project: Arc<Project>,
    path: PathBuf,
    overwrite: bool,
    work: Work,
}

impl Order {
    #[must_use]
    pub fn video(project: Arc<Project>, span: Span, video: Video, path: PathBuf) -> Order {
        Order::new(project, path, Work::Video { video, span })
    }

    #[must_use]
    pub fn audio(project: Arc<Project>, span: Span, audio: Audio, path: PathBuf) -> Order {
        Order::new(project, path, Work::Audio { audio, span })
    }

    #[must_use]
    pub fn still(project: Arc<Project>, frame: i64, still: Still, path: PathBuf) -> Order {
        let frame = frame.max(0);
        Order::new(project, path, Work::Still { still, frame })
    }

    fn new(project: Arc<Project>, path: PathBuf, work: Work) -> Order {
        Order {
            project,
            path,
            overwrite: false,
            work,
        }
    }

    #[must_use]
    pub fn overwriting(mut self) -> Order {
        self.overwrite = true;
        self
    }
    #[must_use]
    pub fn total(&self) -> u64 {
        let rate = self.project.rate();
        match &self.work {
            Work::Video { video, span } => {
                Plan::new(*span, rate, video.frame_rate.rate(rate)).frames()
            }
            Work::Audio { span, .. } => Plan::new(*span, rate, rate).frames(),
            Work::Still { .. } => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub path: PathBuf,
    pub duration: Duration,
    pub frames: u64,
    pub backend: Option<EncoderBackend>,
    pub sound: bool,
    pub undecodable: usize,
    pub offline: usize,
}

#[derive(Debug)]
pub enum Report {
    Opened { backend: Option<EncoderBackend> },
    Advanced { done: u64 },
    Previewed(ComputedFrame),
    Finishing,
    Finished(Summary),
    Failed(Error),
    Cancelled,
}

#[derive(Clone, Debug)]
pub enum Phase {
    Preparing,
    Rendering,
    Finishing,
    Cancelling,
    Done(Summary),
    Failed(Arc<Error>),
    Cancelled,
}

struct Stopping(CancelToken);

impl Stop for Stopping {
    fn stop(&self) {
        self.0.cancel();
    }
}

pub struct Job {
    total: u64,
    done: u64,
    started: Option<Instant>,
    backend: Option<EncoderBackend>,
    phase: Phase,
    worker: Worker<Stopping>,
}

impl Job {
    pub fn start(
        order: Order,
        imaging: Imaging,
        report: impl Fn(Report) + Send + Sync + 'static,
    ) -> Result<Job> {
        let total = order.total();
        let cancel = CancelToken::new();
        let stopping = cancel.clone();
        let report: Reporter = Arc::new(report);
        let worker = Worker::spawn("rikka-cut-export", Stopping(cancel), move || {
            Ok::<_, Infallible>(move || run(&order, &imaging, &report, &stopping))
        })?;
        Ok(Job {
            total,
            done: 0,
            started: None,
            backend: None,
            phase: Phase::Preparing,
            worker,
        })
    }

    pub fn hear(&mut self, report: Report) {
        match report {
            Report::Opened { backend } => {
                self.backend = backend;
                self.started = Some(Instant::now());
                if matches!(self.phase, Phase::Preparing) {
                    self.phase = Phase::Rendering;
                }
            }
            Report::Advanced { done } => self.done = done,
            Report::Previewed(_) => {}
            Report::Finishing => {
                if matches!(self.phase, Phase::Rendering) {
                    self.phase = Phase::Finishing;
                }
            }
            Report::Finished(summary) => self.phase = Phase::Done(summary),
            Report::Failed(error) => self.phase = Phase::Failed(Arc::new(error)),
            Report::Cancelled => self.phase = Phase::Cancelled,
        }
        if self.is_over() {
            self.worker.stop();
        }
    }

    pub fn cancel(&mut self) {
        if self.can_cancel() {
            self.worker.handle().0.cancel();
            self.phase = Phase::Cancelling;
        }
    }

    #[must_use]
    pub fn can_cancel(&self) -> bool {
        matches!(self.phase, Phase::Preparing | Phase::Rendering)
    }

    #[must_use]
    pub fn is_over(&self) -> bool {
        matches!(
            self.phase,
            Phase::Done(_) | Phase::Failed(_) | Phase::Cancelled
        )
    }

    #[must_use]
    pub fn phase(&self) -> &Phase {
        &self.phase
    }
    #[must_use]
    pub fn done(&self) -> u64 {
        self.done
    }

    #[must_use]
    pub fn total(&self) -> u64 {
        self.total
    }

    #[must_use]
    pub fn fraction(&self) -> f32 {
        (self.done as f64 / self.total.max(1) as f64) as f32
    }

    #[must_use]
    pub fn backend(&self) -> Option<EncoderBackend> {
        self.backend
    }

    #[must_use]
    pub fn pace(&self) -> Option<f64> {
        let elapsed = self.started?.elapsed().as_secs_f64();
        (self.done > 0 && elapsed > 0.0).then(|| self.done as f64 / elapsed)
    }

    #[must_use]
    pub fn remaining(&self) -> Option<Duration> {
        let pace = self.pace()?;
        Some(Duration::from_secs_f64(
            (self.total - self.done.min(self.total)) as f64 / pace,
        ))
    }
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job")
            .field("done", &self.done)
            .field("total", &self.total)
            .field("phase", &self.phase)
            .finish_non_exhaustive()
    }
}

struct Progress {
    report: Reporter,
    total: u64,
    last_report: Option<Instant>,
}

impl Progress {
    fn new(report: &Reporter, total: u64) -> Progress {
        Progress {
            report: Arc::clone(report),
            total,
            last_report: None,
        }
    }

    fn tick(&mut self, done: u64) {
        if done == self.total
            || self
                .last_report
                .is_none_or(|last| last.elapsed() >= PROGRESS_EVERY)
        {
            self.last_report = Some(Instant::now());
            (self.report)(Report::Advanced { done });
        }
    }
}

fn run(order: &Order, imaging: &Imaging, report: &Reporter, cancel: &CancelToken) {
    let project = &order.project;
    let output = match OutputFile::new(&order.path, order.overwrite) {
        Ok(output) => output,
        Err(trouble) => return report(Report::Failed(trouble.into())),
    };
    let ended = match &order.work {
        Work::Video { video, span } => film::film(
            &film::Take {
                project,
                span: *span,
                video,
                imaging,
            },
            output,
            report,
            cancel,
        ),
        Work::Audio { audio, span } => tape::tape(project, *span, audio, output, report, cancel),
        Work::Still { still, frame } => {
            still::still(project, *frame, still, output, imaging, report, cancel)
        }
    };
    report(match ended {
        Ok(Some(summary)) => Report::Finished(summary),
        Ok(None) => Report::Cancelled,
        Err(trouble) => Report::Failed(trouble),
    });
}
