use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use rikka_audio_decode::Sound;
use rikka_renderer::Readback;
use rikka_types::PixelSize;
use rikka_video::{EncodedVideo, VideoEncoder};

use super::plan::Plan;
use super::preview::preview;
use super::{ExportError, Report, Reporter};

const PROGRESS_EVERY: Duration = Duration::from_millis(100);
const PREVIEW_EVERY: Duration = Duration::from_millis(400);

pub(crate) struct Encoder {
    pub(crate) encoder: VideoEncoder,
    pub(crate) plan: Plan,
    pub(crate) sound: Option<Sound>,
    pub(crate) size: PixelSize,
    pub(crate) report: Reporter,
    pub(crate) cancel: Arc<AtomicBool>,
}

impl Encoder {
    pub(crate) fn run(
        mut self,
        shots: &Receiver<Readback>,
        returned: &Sender<Readback>,
    ) -> Result<Option<EncodedVideo>, ExportError> {
        let total = self.plan.frames();
        let PixelSize { width, height } = self.size;
        let mut pushed = 0;
        let mut samples = Vec::new();
        let mut reported_at: Option<Instant> = None;
        let mut last_preview: Option<Instant> = None;
        for readback in shots {
            if self.cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            self.push_sound(pushed, &mut samples)?;
            self.encoder.push_frame(readback.pixels())?;
            pushed += 1;
            let last = pushed == total;
            if last || reported_at.is_none_or(|reported_at| reported_at.elapsed() >= PROGRESS_EVERY)
            {
                reported_at = Some(Instant::now());
                (self.report)(Report::Advanced { frames: pushed });
            }
            if last || last_preview.is_none_or(|at| at.elapsed() >= PREVIEW_EVERY) {
                last_preview = Some(Instant::now());
                (self.report)(Report::Previewed(preview(readback.pixels(), width, height)));
            }
            let _ = returned.send(readback);
        }
        if self.cancel.load(Ordering::Relaxed) || pushed < total {
            return Ok(None);
        }
        (self.report)(Report::Finishing);
        Ok(Some(self.encoder.finish()?))
    }

    fn push_sound(&mut self, index: u64, samples: &mut Vec<f32>) -> rikka_video::Result<()> {
        let Some(sound) = &self.sound else {
            return Ok(());
        };
        let channels = usize::from(sound.channels());
        let range = self.plan.samples(index, sound.sample_rate());
        let all = sound.samples();
        let from = (range.start as usize * channels).min(all.len());
        let to = (range.end as usize * channels).min(all.len());
        samples.clear();
        samples.extend_from_slice(&all[from..to]);
        samples.resize((range.end - range.start) as usize * channels, 0.0);
        self.encoder.push_audio(samples)
    }
}
