use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, AtomicUsize, Ordering, fence};
use std::time::Duration;

use crate::atomic::AtomicF64;
use crate::error::{Result, Role};
use crate::frame::{Frame, MAX_CHANNELS, check_channels, interpolate};
use crate::play_params::PlayParams;
use crate::spawn;
use crate::voice::VoiceShared;
use rikka_audio_decode::Decoder;
use rikka_resample::{ResampleQuality, Resampler};
use rikka_ring::{Consumer, Producer};
use rikka_worker::{EventFd, EventNotifier, Stop, Worker, event_fd};

const RING_SECONDS: f64 = 0.5;
const MIN_RING_FRAMES: usize = 1024;

const STREAM_NORMAL: u8 = 0;
const STREAM_SEEK_REQUESTED: u8 = 1;
const STREAM_FLUSHING: u8 = 2;

const DECODER_THREAD: &str = "rikka-audio-stream";

#[derive(Debug)]
pub(crate) struct StreamShared {
    state: AtomicU8,
    seek_seconds: AtomicF64,
    seeks: AtomicU64,
    base_seconds: AtomicF64,
    consumed_frames: AtomicU64,
    flushes: AtomicU64,
    ended: AtomicBool,
    closed: AtomicBool,
    underruns: AtomicU32,
    output_rate: u32,
    duration: Option<Duration>,
    producer: EventFd,
    producer_bell: EventNotifier,
    room_wanted: AtomicUsize,
}

impl StreamShared {
    pub(crate) fn new(output_rate: u32, duration: Option<Duration>) -> std::io::Result<Self> {
        let (producer_bell, producer) = event_fd()?;
        Ok(StreamShared {
            state: AtomicU8::new(STREAM_NORMAL),
            seek_seconds: AtomicF64::new(0.0),
            seeks: AtomicU64::new(0),
            base_seconds: AtomicF64::new(0.0),
            consumed_frames: AtomicU64::new(0),
            flushes: AtomicU64::new(0),
            ended: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            underruns: AtomicU32::new(0),
            output_rate,
            duration,
            producer,
            producer_bell,
            room_wanted: AtomicUsize::new(0),
        })
    }

    pub(crate) fn notify_producer(&self) {
        if let Err(error) = self.producer_bell.notify() {
            tracing::warn!("failed to wake the audio producer: {error}");
        }
    }

    pub(crate) fn wait_for_producer(&self, timeout: Option<Duration>) {
        let waited = match timeout {
            Some(timeout) => self.producer.wait_timeout(timeout),
            None => self.producer.wait(),
        };
        if let Err(error) = waited {
            tracing::warn!("failed to wait for the audio callback: {error}");
        }
        self.producer.drain();
    }

    pub(crate) fn wait_for_room(
        &self,
        ring: &Producer<f32>,
        needed: usize,
        timeout: Option<Duration>,
    ) {
        self.room_wanted.store(needed, Ordering::SeqCst);
        fence(Ordering::SeqCst);
        if ring.free() < needed {
            self.wait_for_producer(timeout);
        }
        self.room_wanted.store(0, Ordering::SeqCst);
    }

    pub(crate) fn arm_room(&self, ring: &Producer<f32>, needed: usize) {
        self.producer.drain();
        self.room_wanted.store(needed, Ordering::SeqCst);
        fence(Ordering::SeqCst);
        if ring.free() >= needed {
            self.notify_producer();
        }
    }

    pub(crate) fn producer(&self) -> &EventFd {
        &self.producer
    }

    fn release_room(&self, free: usize) {
        fence(Ordering::SeqCst);
        let needed = self.room_wanted.load(Ordering::SeqCst);
        if needed != 0
            && free >= needed
            && self
                .room_wanted
                .compare_exchange(needed, 0, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
        {
            self.notify_producer();
        }
    }

    pub(crate) fn request_seek(&self, seconds: f64) {
        self.seek_seconds.set(seconds.max(0.0));
        self.seeks.fetch_add(1, Ordering::AcqRel);
        self.state.store(STREAM_SEEK_REQUESTED, Ordering::Release);
        self.notify_producer();
    }

    pub(crate) fn request_flush(&self) {
        self.base_seconds.set(self.position().as_secs_f64());
        self.flushes.fetch_add(1, Ordering::AcqRel);
        self.state.store(STREAM_FLUSHING, Ordering::Release);
        self.notify_producer();
    }

    pub(crate) fn end(&self) {
        self.ended.store(true, Ordering::Release);
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.notify_producer();
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub(crate) fn flushes(&self) -> u64 {
        self.flushes.load(Ordering::Acquire)
    }

    pub(crate) fn is_flushing(&self) -> bool {
        self.state.load(Ordering::Acquire) == STREAM_FLUSHING
    }
    pub(crate) fn position(&self) -> Duration {
        if self.output_rate == 0 {
            return Duration::ZERO;
        }
        let consumed = self.consumed_frames.load(Ordering::Relaxed);
        let mut seconds = self.base_seconds.get() + consumed as f64 / self.output_rate as f64;
        if let Some(duration) = self.duration {
            let end = duration.as_secs_f64();
            if seconds > end && end > 0.0 {
                seconds %= end;
            }
        }
        Duration::from_secs_f64(seconds.max(0.0))
    }

    pub(crate) fn underruns(&self) -> u32 {
        self.underruns.load(Ordering::Relaxed)
    }

    pub(crate) fn duration(&self) -> Option<Duration> {
        self.duration
    }
}

struct Closing(Arc<StreamShared>);

impl Stop for Closing {
    fn stop(&self) {
        self.0.close();
    }
}

pub(crate) struct StreamSource {
    _decoding: Option<Worker<Closing>>,
    ring: Consumer<f32>,
    shared: Arc<StreamShared>,
    channels: usize,
    history: [Frame; 4],
    history_head: usize,
    history_frac: f64,
    history_primed: bool,
    popped_this_block: usize,
}

impl StreamSource {
    pub(crate) fn open(
        mut decoder: Decoder,
        params: &PlayParams,
        voice: Arc<VoiceShared>,
        output_rate: u32,
        resampler: ResampleQuality,
    ) -> Result<StreamSource> {
        let info = *decoder.info();
        check_channels(info.channels, Role::Source)?;

        let initial_seconds = match params.start_at {
            Some(position) if !position.is_zero() => decoder.seek(position)?.as_secs_f64(),
            _ => 0.0,
        };

        let channels = info.channels.max(1);
        let ring_capacity =
            ((output_rate as f64 * RING_SECONDS) as usize).max(MIN_RING_FRAMES) * channels as usize;
        let (producer, consumer) = rikka_ring::channel::<f32>(ring_capacity);

        let shared = Arc::new(StreamShared::new(output_rate, info.duration)?);
        shared.base_seconds.set(initial_seconds);

        let source_rate = info.sample_rate;
        let worker_shared = shared.clone();
        let decoding = Worker::spawn(DECODER_THREAD, Closing(shared.clone()), move || {
            Ok(move || {
                DecoderWorker {
                    resampler: Resampler::with_quality(
                        resampler,
                        source_rate,
                        output_rate,
                        channels,
                    ),
                    decoder,
                    ring: producer,
                    shared: worker_shared,
                    voice,
                    outbox: Vec::new(),
                    outbox_read: 0,
                    decode_buf: Vec::new(),
                }
                .run();
            })
        })
        .map_err(spawn::failed(DECODER_THREAD))?;

        let mut source = StreamSource::fed(consumer, shared, channels as usize);
        source._decoding = Some(decoding);
        Ok(source)
    }

    pub(crate) fn fed(
        ring: Consumer<f32>,
        shared: Arc<StreamShared>,
        channels: usize,
    ) -> StreamSource {
        StreamSource {
            _decoding: None,
            ring,
            shared,
            channels,
            history: [[0.0; MAX_CHANNELS]; 4],
            history_head: 0,
            history_frac: 0.0,
            history_primed: false,
            popped_this_block: 0,
        }
    }
    pub(crate) fn shared(&self) -> &Arc<StreamShared> {
        &self.shared
    }

    pub(crate) fn channels(&self) -> usize {
        self.channels
    }

    pub(crate) fn service_flush(&mut self) -> bool {
        if self.shared.state.load(Ordering::Acquire) != STREAM_FLUSHING {
            return false;
        }
        self.ring.clear();
        if self.ring.is_empty() {
            self.shared.consumed_frames.store(0, Ordering::Relaxed);
            self.history_primed = false;
            self.history_frac = 0.0;
            let flushed = self.shared.state.compare_exchange(
                STREAM_FLUSHING,
                STREAM_NORMAL,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            self.shared.notify_producer();
            return flushed.is_err();
        }
        true
    }

    pub(crate) fn is_exhausted(&self) -> bool {
        self.shared.ended.load(Ordering::Acquire)
            && self.ring.is_empty()
            && self.shared.state.load(Ordering::Relaxed) == STREAM_NORMAL
    }

    pub(crate) fn read(&mut self, step: f64, out: &mut Frame) -> bool {
        if !self.history_primed {
            self.history = [[0.0; MAX_CHANNELS]; 4];
            self.history_head = 0;
            for slot in 1..4 {
                if !self.pop_into(slot) {
                    return false;
                }
            }
            self.history_frac = 0.0;
            self.history_primed = true;
        }

        while self.history_frac >= 1.0 {
            let slot = self.history_head;
            self.history_head = (self.history_head + 1) & 3;
            if !self.pop_into(slot) {
                self.history_primed = false;
                if !self.shared.ended.load(Ordering::Relaxed) {
                    self.shared.underruns.fetch_add(1, Ordering::Relaxed);
                }
                return false;
            }
            self.history_frac -= 1.0;
        }

        let taps = self.taps();
        interpolate(
            taps[0],
            taps[1],
            taps[2],
            taps[3],
            self.history_frac as f32,
            out,
        );
        self.history_frac += step;
        true
    }

    fn taps(&self) -> [&[f32]; 4] {
        let h = self.history_head;
        [
            &self.history[h][..self.channels],
            &self.history[(h + 1) & 3][..self.channels],
            &self.history[(h + 2) & 3][..self.channels],
            &self.history[(h + 3) & 3][..self.channels],
        ]
    }

    fn pop_into(&mut self, slot: usize) -> bool {
        if self.ring.len() < self.channels {
            return false;
        }
        self.ring
            .pop_slice(&mut self.history[slot][..self.channels]);
        self.popped_this_block += 1;
        true
    }

    pub(crate) fn publish_position(&mut self) {
        if self.popped_this_block > 0 {
            self.shared
                .consumed_frames
                .fetch_add(self.popped_this_block as u64, Ordering::Relaxed);
            self.popped_this_block = 0;
            self.shared.release_room(self.ring.free());
        }
    }
}

impl Drop for StreamSource {
    fn drop(&mut self) {
        self.shared.notify_producer();
    }
}

struct DecoderWorker {
    decoder: Decoder,
    resampler: Resampler,
    ring: Producer<f32>,
    shared: Arc<StreamShared>,
    voice: Arc<VoiceShared>,
    outbox: Vec<f32>,
    outbox_read: usize,
    decode_buf: Vec<f32>,
}

impl DecoderWorker {
    fn run(mut self) {
        loop {
            if self.voice_gone() {
                return;
            }
            match self.shared.state.load(Ordering::Acquire) {
                STREAM_SEEK_REQUESTED => self.handle_seek(),
                STREAM_FLUSHING => self.shared.wait_for_producer(None),
                _ => self.pump(),
            }
        }
    }

    fn voice_gone(&self) -> bool {
        self.shared.is_closed()
            || self.voice.finished.load(Ordering::Relaxed)
            || self.voice.stop.load(Ordering::Relaxed)
    }

    fn wait_for_drain(&self) {
        if self.voice.paused.load(Ordering::Relaxed) {
            self.shared.wait_for_producer(None);
            return;
        }
        let pending = self.outbox.len() - self.outbox_read;
        let half_ring = (self.ring.free() + self.ring.len()) / 2;
        self.shared
            .wait_for_room(&self.ring, pending.min(half_ring), None);
    }

    fn handle_seek(&mut self) {
        let asked = self.shared.seeks.load(Ordering::Acquire);
        let target = self.shared.seek_seconds.get();
        self.outbox.clear();
        self.outbox_read = 0;
        self.resampler.reset();
        let actual = match self.decoder.seek(Duration::from_secs_f64(target.max(0.0))) {
            Ok(actual) => actual.as_secs_f64(),
            Err(err) => {
                tracing::warn!("stream seek failed: {err}");
                target
            }
        };
        if self.shared.seeks.load(Ordering::Acquire) != asked {
            return;
        }
        self.shared.ended.store(false, Ordering::Relaxed);
        self.shared.base_seconds.set(actual);
        self.shared.state.store(STREAM_FLUSHING, Ordering::Release);
        if self.shared.seeks.load(Ordering::Acquire) != asked {
            self.shared
                .state
                .store(STREAM_SEEK_REQUESTED, Ordering::Release);
        }
    }

    fn pump(&mut self) {
        if self.outbox_read < self.outbox.len() {
            let pushed = self.ring.push_slice(&self.outbox[self.outbox_read..]);
            self.outbox_read += pushed;
            if self.outbox_read < self.outbox.len() {
                self.wait_for_drain();
            }
            return;
        }
        self.outbox.clear();
        self.outbox_read = 0;

        if self.shared.ended.load(Ordering::Relaxed) {
            if self.voice.looping.load(Ordering::Relaxed) {
                self.shared.request_seek(0.0);
            } else {
                self.shared.wait_for_producer(None);
            }
            return;
        }

        self.decode_buf.clear();
        match self.decoder.read(&mut self.decode_buf) {
            Ok(true) => {
                self.resampler.process(&self.decode_buf, &mut self.outbox);
            }
            Ok(false) if self.voice.looping.load(Ordering::Relaxed) => {
                if let Err(err) = self.decoder.seek(Duration::ZERO) {
                    tracing::warn!("stream loop seek failed: {err}");
                    self.finish_tail();
                }
            }
            Ok(false) => self.finish_tail(),
            Err(err) => {
                tracing::warn!("stream decode failed: {err}");
                self.finish_tail();
            }
        }
    }

    fn finish_tail(&mut self) {
        self.resampler.flush(&mut self.outbox);
        self.shared.ended.store(true, Ordering::Release);
    }
}
