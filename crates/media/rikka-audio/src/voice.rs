use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::atomic::AtomicF32;
use crate::buffered::BufferedSource;
use crate::frame::{Frame, MAX_CHANNELS};
use crate::gains::{BlockGains, Fold};
use crate::play_params::PlayParams;
use crate::stream::StreamSource;
use crate::watch::Listeners;

pub(crate) const NO_SEEK: u64 = u64::MAX;

pub(crate) const MIN_SPEED: f32 = 0.01;
pub(crate) const MAX_SPEED: f32 = 64.0;

#[derive(Debug)]
pub(crate) struct VoiceShared {
    pub(crate) volume: AtomicF32,
    pub(crate) speed: AtomicF32,
    pub(crate) pan: AtomicF32,
    pub(crate) looping: AtomicBool,
    pub(crate) paused: AtomicBool,
    pub(crate) stop: AtomicBool,
    pub(crate) finished: AtomicBool,
    pub(crate) position_frames: AtomicU64,
    pub(crate) seek_frame: AtomicU64,
    pub(crate) listeners: Listeners,
}

impl From<&PlayParams> for VoiceShared {
    fn from(params: &PlayParams) -> Self {
        VoiceShared {
            volume: AtomicF32::new(params.volume.max(0.0)),
            speed: AtomicF32::new(params.speed.clamp(MIN_SPEED, MAX_SPEED)),
            pan: AtomicF32::new(params.pan.clamp(-1.0, 1.0)),
            looping: AtomicBool::new(params.looping),
            paused: AtomicBool::new(params.paused),
            stop: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            position_frames: AtomicU64::new(0),
            seek_frame: AtomicU64::new(NO_SEEK),
            listeners: Listeners::default(),
        }
    }
}

pub(crate) enum VoiceSource {
    Buffered(BufferedSource),
    Stream(Box<StreamSource>),
}

impl VoiceSource {
    fn channels(&self) -> usize {
        match self {
            VoiceSource::Buffered(buffered) => buffered.channels(),
            VoiceSource::Stream(stream) => stream.channels(),
        }
    }
}

pub(crate) struct Voice {
    pub(crate) shared: Arc<VoiceShared>,
    source: VoiceSource,
    source_rate: u32,
    pub(crate) sequence: u64,
    gains: BlockGains,
    gains_primed: bool,
    idle: bool,
}

pub(crate) enum VoiceStatus {
    Active,
    Finished,
}

impl Voice {
    pub(crate) fn new(
        shared: Arc<VoiceShared>,
        source: VoiceSource,
        source_rate: u32,
        sequence: u64,
    ) -> Self {
        Voice {
            shared,
            source,
            source_rate,
            sequence,
            gains: BlockGains::ZERO,
            gains_primed: false,
            idle: false,
        }
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.idle
    }

    pub(crate) fn is_stealable(&self) -> bool {
        matches!(self.source, VoiceSource::Buffered(_))
            && !self.shared.looping.load(Ordering::Relaxed)
    }

    pub(crate) fn mark_finished(&self) {
        self.shared.finished.store(true, Ordering::Release);
    }

    pub(crate) fn render(
        &mut self,
        mix: &mut [f32],
        out_channels: usize,
        output_rate: u32,
    ) -> VoiceStatus {
        let frames = mix.len() / out_channels;

        let stopping = self.shared.stop.load(Ordering::Relaxed);
        let paused = self.shared.paused.load(Ordering::Relaxed);
        let looping = self.shared.looping.load(Ordering::Relaxed);
        let volume = self.shared.volume.get();
        let speed = f64::from(self.shared.speed.get());
        let pan = self.shared.pan.get();
        let source_channels = self.source.channels();

        self.apply_seek();
        let flushing = self.service_stream_flush();

        let audible = !paused && !stopping && !flushing;
        let target = if audible {
            BlockGains::target(volume, pan, source_channels == 1)
        } else {
            BlockGains::ZERO
        };
        if !self.gains_primed {
            self.gains = target;
            self.gains_primed = true;
        }

        self.idle = false;
        if !audible && self.gains.is_silent() {
            self.gains = target;
            self.idle = paused && !stopping && !flushing;
            return if stopping {
                self.finish()
            } else {
                VoiceStatus::Active
            };
        }

        let step = speed * f64::from(self.source_rate) / f64::from(output_rate);
        let destination = &mut mix[..frames * out_channels];

        let fold = Fold::select(source_channels, out_channels);
        let mut gains = self.gains;
        let gain_step = gains.step_towards(&target, frames);
        let mut sample: Frame = [0.0; MAX_CHANNELS];

        let mut source_ended = false;
        for out_frame in destination.chunks_exact_mut(out_channels) {
            let have_frame = match &mut self.source {
                VoiceSource::Buffered(buffered) => buffered.read(step, looping, &mut sample),
                VoiceSource::Stream(stream) => stream.read(step, &mut sample),
            };
            if !have_frame {
                source_ended = matches!(self.source, VoiceSource::Buffered(_));
                break;
            }
            gains.advance(&gain_step);
            gains.mix_frame(out_frame, &sample, source_channels, fold);
        }
        self.gains = target;

        self.publish_position();

        if stopping || (source_ended && !looping) || self.stream_exhausted(looping) {
            return self.finish();
        }
        VoiceStatus::Active
    }

    fn finish(&mut self) -> VoiceStatus {
        self.mark_finished();
        VoiceStatus::Finished
    }

    fn apply_seek(&mut self) {
        let target = self.shared.seek_frame.swap(NO_SEEK, Ordering::Relaxed);
        if target == NO_SEEK {
            return;
        }
        if let VoiceSource::Buffered(buffered) = &mut self.source {
            let frame = buffered.seek(target);
            self.shared.position_frames.store(frame, Ordering::Relaxed);
        }
    }

    fn service_stream_flush(&mut self) -> bool {
        let VoiceSource::Stream(stream) = &mut self.source else {
            return false;
        };
        stream.service_flush()
    }

    fn stream_exhausted(&self, looping: bool) -> bool {
        match &self.source {
            VoiceSource::Stream(stream) => !looping && stream.is_exhausted(),
            VoiceSource::Buffered(_) => false,
        }
    }

    fn publish_position(&mut self) {
        match &mut self.source {
            VoiceSource::Buffered(buffered) => {
                self.shared
                    .position_frames
                    .store(buffered.position_frames(), Ordering::Relaxed);
            }
            VoiceSource::Stream(stream) => stream.publish_position(),
        }
    }
}

impl Drop for Voice {
    fn drop(&mut self) {
        self.mark_finished();
        self.shared.listeners.finish();
    }
}
