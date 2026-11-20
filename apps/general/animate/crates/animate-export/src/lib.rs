mod encode;
mod encoding;
mod error;
mod export;
mod labels;
mod plan;
mod preview;
mod render;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use animate_stage::Replica;
use rikka_audio_decode::Sound;
use rikka_ui::ImageSource;
use rikka_video::EncoderBackend;

pub use encoding::{
    BITRATES, CODECS, CONTAINERS, Container, ENCODERS, Encoding, FRAME_RATES, FrameRate, QUALITIES,
    RATES, Rate, SOUND_BITRATES, SPEEDS, name_of, named, position,
};
pub use error::ExportError;
pub use export::Export;
pub use labels::{
    backend_label, codec_label, container_label, encoder_label, rate_label, speed_label,
};
pub use plan::Plan;

pub type Reporter = Arc<dyn Fn(Report) + Send + Sync>;

#[derive(Debug)]
pub struct Request {
    pub stage: Replica,
    pub export: Export,
    pub sound: Option<Sound>,
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub path: PathBuf,
    pub frames: u64,
    pub duration: Duration,
    pub sound: bool,
    pub backend: EncoderBackend,
}

#[derive(Debug)]
pub enum Report {
    Opened { backend: EncoderBackend },
    Advanced { frames: u64 },
    Previewed(ImageSource),
    Finishing,
    Finished(Summary),
    Failed(ExportError),
    Cancelled,
}

#[derive(Debug)]
pub enum Phase {
    Preparing,
    Rendering,
    Finishing,
    Cancelling,
    Done(Summary),
    Failed(ExportError),
}

pub struct Job {
    path: PathBuf,
    total: u64,
    frames: u64,
    started: Option<Instant>,
    backend: Option<EncoderBackend>,
    preview: Option<ImageSource>,
    phase: Phase,
    cancel: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Job {
    pub fn start(request: Request, report: Reporter) -> Result<Self, ExportError> {
        let total = Plan::for_export(&request.export).frames();
        let path = request.path.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&cancel);
        let worker = std::thread::Builder::new()
            .name("rikka-animate-export".to_owned())
            .spawn(move || render::run(request, &report, &stopping))
            .map_err(ExportError::Thread)?;
        Ok(Self {
            path,
            total,
            frames: 0,
            started: None,
            backend: None,
            preview: None,
            phase: Phase::Preparing,
            cancel,
            worker: Some(worker),
        })
    }

    pub fn apply_report(&mut self, report: Report) {
        match report {
            Report::Opened { backend } => {
                self.backend = Some(backend);
                self.started = Some(Instant::now());
                self.phase = Phase::Rendering;
            }
            Report::Advanced { frames } => self.frames = frames,
            Report::Previewed(preview) => self.preview = Some(preview),
            Report::Finishing => self.phase = Phase::Finishing,
            Report::Finished(summary) => self.phase = Phase::Done(summary),
            Report::Failed(error) => self.phase = Phase::Failed(error),
            Report::Cancelled => {}
        }
        if self.is_over()
            && let Some(worker) = self.worker.take()
        {
            let _ = worker.join();
        }
    }

    pub fn cancel(&mut self) {
        if self.can_cancel() {
            self.cancel.store(true, Ordering::Relaxed);
            self.phase = Phase::Cancelling;
        }
    }

    pub fn can_cancel(&self) -> bool {
        matches!(self.phase, Phase::Preparing | Phase::Rendering)
    }

    pub fn is_over(&self) -> bool {
        matches!(self.phase, Phase::Done(_) | Phase::Failed(_))
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    pub fn fraction(&self) -> f32 {
        (self.frames as f64 / self.total.max(1) as f64) as f32
    }

    pub fn backend(&self) -> Option<EncoderBackend> {
        self.backend
    }

    pub fn preview(&self) -> Option<&ImageSource> {
        self.preview.as_ref()
    }

    pub fn pace(&self) -> Option<f64> {
        let elapsed = self.started?.elapsed().as_secs_f64();
        (self.frames > 0 && elapsed > 0.0).then(|| self.frames as f64 / elapsed)
    }

    pub fn remaining(&self) -> Option<Duration> {
        let pace = self.pace()?;
        Some(Duration::from_secs_f64(
            (self.total - self.frames.min(self.total)) as f64 / pace,
        ))
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job")
            .field("path", &self.path)
            .field("frames", &self.frames)
            .field("total", &self.total)
            .field("phase", &self.phase)
            .finish_non_exhaustive()
    }
}

pub static CATALOGS: &[(&str, &str)] = rikka_i18n::catalogs!();
