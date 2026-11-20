use std::convert::Infallible;
use std::os::fd::AsFd;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rikka_audio::{AudioCapture, CaptureConfig, CaptureSource, Fill};
use rikka_video::{AudioCodec, AudioConfig, Coded, SoundPackets};
use rikka_worker::{CommandReceiver, CommandSender, Worker, command_channel};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::io::Errno;

use crate::clock;
use crate::error::{Error, Result};
use crate::options::AudioOptions;
use crate::store::{DESKTOP, MICROPHONE, SharedStore};

const BITRATE: u64 = 160_000;
const BLOCK_DURATION: Duration = Duration::from_millis(20);
const SILENCE_THRESHOLD: Duration = Duration::from_millis(150);
const SILENCE_SLACK: Duration = Duration::from_millis(50);
const RETRY_INTERVAL: Duration = Duration::from_secs(5);
const RATE: u32 = 48_000;
const CHANNELS: u16 = 2;

const THREAD: &str = "rikka-replay-sound";

enum Command {
    Stop,
}

#[derive(Debug)]
pub(crate) struct Sound {
    _worker: Worker<CommandSender<Command>>,
}

impl Sound {
    pub(crate) fn start(audio: AudioOptions, store: SharedStore) -> Result<Sound> {
        let (commands, receiver) = command_channel(|| Command::Stop).map_err(Error::Workers)?;
        let worker = Worker::spawn(THREAD, commands, move || {
            Ok::<_, Infallible>(move || capture_audio(&audio, &store, &receiver))
        })
        .map_err(|error| match error {
            rikka_worker::Error::Spawn(error) => Error::Workers(error),
            rikka_worker::Error::Startup(never) => match never {},
            rikka_worker::Error::Exited => Error::Workers(std::io::Error::other(
                "the replay's sound worker stopped before it began",
            )),
        })?;
        Ok(Sound { _worker: worker })
    }
}

struct AudioTrack {
    capture: AudioCapture,
    coder: SoundPackets,
    track: usize,
    rate: u32,
    channels: usize,
    origin_us: i64,
    frames: u64,
    block: Vec<f32>,
}

impl AudioTrack {
    fn open(source: CaptureSource, track: usize) -> Result<AudioTrack> {
        let preferred = CaptureConfig::default()
            .source(source.clone())
            .sample_rate(RATE)
            .channels(CHANNELS);
        let capture = AudioCapture::start(preferred)
            .or_else(|_| AudioCapture::start(CaptureConfig::default().source(source)))?;
        let (rate, channels) = (capture.sample_rate(), usize::from(capture.channels()));
        let config = AudioConfig::new(rate, capture.channels())
            .codec(AudioCodec::Aac)
            .bitrate(BITRATE);
        let coder = SoundPackets::open(config)?;
        let frames_per_block =
            (u64::from(rate) * BLOCK_DURATION.as_millis() as u64 / 1000) as usize;
        Ok(AudioTrack {
            capture,
            coder,
            track,
            rate,
            channels,
            origin_us: clock::now_us(),
            frames: 0,
            block: vec![0.0; frames_per_block * channels],
        })
    }

    fn pump(&mut self, store: &SharedStore) -> bool {
        loop {
            match self.capture.fill(&mut self.block, Duration::ZERO) {
                Fill::Filled => {
                    let block = std::mem::take(&mut self.block);
                    let coded = self.encode(&block, store);
                    self.block = block;
                    if !coded {
                        return false;
                    }
                }
                Fill::Silent => return self.fill_silence(store),
                Fill::Ended => return false,
            }
        }
    }

    fn heard_until(&self) -> i64 {
        self.origin_us + (self.frames * 1_000_000 / u64::from(self.rate)) as i64
    }

    fn silence_due(&self) -> Instant {
        let late = self.heard_until() + SILENCE_THRESHOLD.as_micros() as i64 - clock::now_us();
        Instant::now() + Duration::from_micros(late.max(0) as u64)
    }

    fn fill_silence(&mut self, store: &SharedStore) -> bool {
        let lag = clock::now_us() - self.heard_until();
        if lag <= SILENCE_THRESHOLD.as_micros() as i64 {
            return true;
        }
        let missing =
            (lag - SILENCE_SLACK.as_micros() as i64) as u64 * u64::from(self.rate) / 1_000_000;
        let silence = vec![0.0; missing as usize * self.channels];
        self.encode(&silence, store)
    }

    fn encode(&mut self, samples: &[f32], store: &SharedStore) -> bool {
        self.frames += (samples.len() / self.channels) as u64;
        match self.coder.push(samples) {
            Ok(coded) => {
                self.keep(coded, store);
                true
            }
            Err(error) => {
                tracing::warn!("cannot encode audio: {error}");
                false
            }
        }
    }

    fn keep(&self, coded: Vec<Coded>, store: &SharedStore) {
        if coded.is_empty() {
            return;
        }
        let mut store = store.lock();
        for packet in coded {
            store.ring.push_sound(
                self.track,
                Coded {
                    keyframe: true,
                    ..packet.shifted(-self.origin_us)
                },
            );
        }
    }

    fn register_stream(&self, store: &SharedStore) {
        match self.coder.stream() {
            Ok(stream) => store
                .lock()
                .set_sound_stream(self.track, Some(Arc::new(stream))),
            Err(error) => tracing::warn!("cannot describe audio stream: {error}"),
        }
    }
}

struct TrackSlot {
    track_capture: Option<AudioTrack>,
    source: CaptureSource,
    track: usize,
    retry_at: Option<Instant>,
}

impl TrackSlot {
    fn new(source: CaptureSource, track: usize) -> TrackSlot {
        TrackSlot {
            track_capture: None,
            source,
            track,
            retry_at: None,
        }
    }

    fn ensure_open(&mut self, store: &SharedStore) {
        if self.track_capture.is_some() || self.retry_at.is_some_and(|at| Instant::now() < at) {
            return;
        }
        match AudioTrack::open(self.source.clone(), self.track) {
            Ok(track_capture) => {
                track_capture.register_stream(store);
                self.track_capture = Some(track_capture);
                self.retry_at = None;
            }
            Err(error) => {
                tracing::warn!("audio track {} unavailable: {error}", self.track);
                self.retry_at = Some(Instant::now() + RETRY_INTERVAL);
            }
        }
    }

    fn pump(&mut self, store: &SharedStore) {
        let Some(track_capture) = &mut self.track_capture else {
            return;
        };
        if !track_capture.pump(store) {
            self.track_capture = None;
            self.retry_at = Some(Instant::now() + RETRY_INTERVAL);
        }
    }

    fn deadline(&self) -> Option<Instant> {
        match &self.track_capture {
            Some(track_capture) => Some(track_capture.silence_due()),
            None => self.retry_at,
        }
    }
}

fn microphone(device: String) -> CaptureSource {
    match device.is_empty() {
        true => CaptureSource::Microphone,
        false => CaptureSource::Named(device),
    }
}

fn capture_audio(audio: &AudioOptions, store: &SharedStore, commands: &CommandReceiver<Command>) {
    let desktop = audio
        .desktop
        .then(|| TrackSlot::new(CaptureSource::SystemAudio, DESKTOP));
    let microphone = audio
        .microphone
        .clone()
        .map(|device| TrackSlot::new(microphone(device), MICROPHONE));
    let mut slots: Vec<TrackSlot> = desktop.into_iter().chain(microphone).collect();
    loop {
        if commands.drain().any(|Command::Stop| true) {
            return;
        }
        for slot in &mut slots {
            slot.ensure_open(store);
            slot.pump(store);
        }
        let deadline = slots.iter().filter_map(TrackSlot::deadline).min();
        if let Err(error) = wait(commands, &slots, deadline) {
            tracing::warn!("cannot wait for captured sound: {error}");
            return;
        }
    }
}

fn wait(
    commands: &CommandReceiver<Command>,
    slots: &[TrackSlot],
    deadline: Option<Instant>,
) -> std::io::Result<()> {
    let mut watched: Vec<PollFd<'_>> = std::iter::once(commands.as_fd())
        .chain(
            slots
                .iter()
                .filter_map(|slot| slot.track_capture.as_ref())
                .map(|track| track.capture.as_fd()),
        )
        .map(|fd| PollFd::from_borrowed_fd(fd, PollFlags::IN))
        .collect();
    loop {
        let timeout = deadline.map(|deadline| {
            let left = deadline.saturating_duration_since(Instant::now());
            Timespec::try_from(left).unwrap_or(Timespec {
                tv_sec: i64::MAX,
                tv_nsec: 0,
            })
        });
        match poll(&mut watched, timeout.as_ref()) {
            Ok(_) => return Ok(()),
            Err(Errno::INTR) => {}
            Err(error) => return Err(error.into()),
        }
    }
}
