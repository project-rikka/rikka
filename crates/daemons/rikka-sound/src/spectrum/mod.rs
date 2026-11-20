mod analysis;
mod levels;

use std::sync::Arc;
use std::time::{Duration, Instant};

use rikka_audio::{AudioCapture, CaptureConfig, CaptureSource};
use rikka_sync::Mutex;

use crate::error::{Error, Result};
use analysis::Analysis;

pub(crate) const NAME: &str = "rikka-spectrum";

const STALE: Duration = Duration::from_millis(250);
const DRAINED: usize = 1024;

#[derive(Clone)]
pub struct Spectrum(Arc<Listening>);

struct Listening {
    capture: AudioCapture,
    bands: usize,
    heard: Mutex<Heard>,
}

struct Heard {
    analysis: Analysis,
    drained: Vec<f32>,
    levels: Vec<f32>,
    last: Option<Instant>,
}

impl Spectrum {
    pub fn listen(bands: usize) -> Result<Spectrum> {
        let bands = bands.max(1);
        let capture = AudioCapture::start(
            CaptureConfig::default()
                .source(CaptureSource::SystemAudio)
                .channels(1)
                .node_name(NAME)
                .passive()
                .buffered(STALE),
        )
        .map_err(|error| Error::Unreachable(error.to_string()))?;
        let heard = Heard {
            analysis: Analysis::new(bands, capture.sample_rate()),
            drained: vec![0.0; DRAINED],
            levels: vec![0.0; bands],
            last: None,
        };
        Ok(Spectrum(Arc::new(Listening {
            capture,
            bands,
            heard: Mutex::new(heard),
        })))
    }

    #[must_use]
    pub fn bands(&self) -> usize {
        self.0.bands
    }

    pub fn read(&self, into: &mut [f32]) -> bool {
        if into.is_empty() {
            return false;
        }
        let mut heard = self.0.heard.lock();
        let Heard {
            analysis,
            drained,
            levels,
            last,
        } = &mut *heard;
        loop {
            let taken = self.0.capture.read(drained);
            if taken == 0 {
                break;
            }
            *last = Some(Instant::now());
            if let Some(analysed) = analysis.hear(drained[..taken].iter().copied()) {
                levels.copy_from_slice(analysed);
            }
        }
        if last.is_none_or(|last| last.elapsed() >= STALE) {
            into.fill(0.0);
            return false;
        }
        levels::spread(levels, into);
        true
    }

    #[must_use]
    pub fn is(&self, other: &Spectrum) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for Spectrum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Spectrum")
            .field("bands", &self.bands())
            .finish_non_exhaustive()
    }
}
