use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use rikka_audio::{AudioCapture, CaptureConfig, CaptureSource};
use rikka_capture::{BufferPool, Captured, Constraints, Frames, Region, Screen, Source};
use rikka_gpu::Gpu;
use rikka_video::{
    AudioConfig, EncodeSpeed, EncoderBackend, EncoderChoice, EncoderConfig, Feed, Imported,
    Picture, PixelLayout, RateControl, VideoCodec, VideoEncoder,
};
use rikka_worker::{CommandReceiver, CommandSender, Worker, command_channel};

use crate::error::{Error, Result};

const THREAD: &str = "rikka-record";
const PREFERRED_RATE: u32 = 48_000;
const PREFERRED_CHANNELS: u16 = 2;
const BUFFERS: usize = 3;
const AUDIO_HELD: Duration = Duration::from_secs(1);
const AUDIO_DRAIN: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, Default)]
pub struct AudioTrack {
    pub source: CaptureSource,
    pub bitrate: Option<u64>,
}

impl AudioTrack {
    #[must_use]
    pub fn new(source: CaptureSource) -> AudioTrack {
        AudioTrack {
            source,
            bitrate: None,
        }
    }

    #[must_use]
    pub fn bitrate(mut self, bits_per_second: u64) -> AudioTrack {
        self.bitrate = Some(bits_per_second);
        self
    }
}

#[derive(Clone, Debug)]
pub struct RecordConfig {
    pub fps: u32,
    pub codec: VideoCodec,
    pub backend: EncoderChoice,
    pub rate: RateControl,
    pub speed: EncodeSpeed,
    pub cursor: bool,
    pub region: Option<Region>,
    pub audio: Option<AudioTrack>,
}

impl Default for RecordConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            codec: VideoCodec::H264,
            backend: EncoderChoice::Auto,
            rate: RateControl::Quality(70),
            speed: EncodeSpeed::default(),
            cursor: true,
            region: None,
            audio: None,
        }
    }
}

impl RecordConfig {
    pub fn fps(mut self, fps: u32) -> Self {
        self.fps = fps.max(1);
        self
    }

    pub fn codec(mut self, codec: VideoCodec) -> Self {
        self.codec = codec;
        self
    }

    pub fn backend(mut self, backend: impl Into<EncoderChoice>) -> Self {
        self.backend = backend.into();
        self
    }

    pub fn quality(mut self, quality: u8) -> Self {
        self.rate = RateControl::Quality(quality);
        self
    }

    pub fn speed(mut self, speed: EncodeSpeed) -> Self {
        self.speed = speed;
        self
    }

    pub fn bitrate(mut self, bits_per_second: u64) -> Self {
        self.rate = RateControl::Bitrate(bits_per_second);
        self
    }

    pub fn cursor(mut self, include: bool) -> Self {
        self.cursor = include;
        self
    }

    pub fn region(mut self, region: Region) -> Self {
        self.region = Some(region);
        self
    }

    pub fn audio(mut self, track: AudioTrack) -> Self {
        self.audio = Some(track);
        self
    }
}

#[derive(Clone, Debug)]
pub struct RecordSummary {
    pub path: PathBuf,
    pub frames: u64,
    pub duration: Duration,
    pub audio: bool,
}

enum Command {
    Stop,
}

pub struct Recording {
    worker: Worker<CommandSender<Command>>,
    summary: Receiver<Result<RecordSummary>>,
    ended: Arc<AtomicBool>,
    frames: Arc<AtomicU64>,
}

impl Recording {
    pub fn start(
        source: Source,
        path: impl AsRef<Path>,
        config: RecordConfig,
        ended: impl FnOnce() + Send + 'static,
    ) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let finished = Arc::new(AtomicBool::new(false));
        let frames = Arc::new(AtomicU64::new(0));
        let (commands, receiver) = command_channel(|| Command::Stop).map_err(Error::Thread)?;
        let (report, summary) = mpsc::sync_channel(1);
        let thread_finished = Arc::clone(&finished);
        let thread_frames = Arc::clone(&frames);
        let worker = Worker::spawn(THREAD, commands, move || {
            let session = Session::open(&source, &path, &config, receiver)?;
            Ok::<_, Error>(move || {
                let summary = session.run(&thread_frames);
                thread_finished.store(true, Ordering::Release);
                if report.send(summary).is_err() {
                    tracing::debug!("the recording was dropped before it finished");
                }
                ended();
            })
        })
        .map_err(|error| match error {
            rikka_worker::Error::Spawn(error) => Error::Thread(error),
            rikka_worker::Error::Startup(error) => error,
            rikka_worker::Error::Exited => Error::ThreadDied,
        })?;
        Ok(Self {
            worker,
            summary,
            ended: finished,
            frames,
        })
    }

    pub fn frames(&self) -> u64 {
        self.frames.load(Ordering::Relaxed)
    }

    pub fn is_finished(&self) -> bool {
        self.ended.load(Ordering::Acquire)
    }

    pub fn stop(self) -> Result<RecordSummary> {
        self.worker.stop();
        self.summary.recv().unwrap_or(Err(Error::ThreadDied))
    }
}

impl std::fmt::Debug for Recording {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recording")
            .field("frames", &self.frames())
            .finish_non_exhaustive()
    }
}

struct Picked {
    encoder: VideoEncoder,
    pool: BufferPool,
    gpu: Option<Gpu>,
}

struct Session {
    screen: Screen,
    frames: Frames,
    commands: CommandReceiver<Command>,
    encoder: VideoEncoder,
    pool: BufferPool,
    gpu: Option<Gpu>,
    audio: Option<AudioCapture>,
    samples: Vec<f32>,
    source_size: (u32, u32),
    area: Region,
    feed: Feed,
    path: PathBuf,
    started: Instant,
}

impl Session {
    fn open(
        source: &Source,
        path: &Path,
        config: &RecordConfig,
        commands: CommandReceiver<Command>,
    ) -> Result<Self> {
        let mut screen = Screen::connect()?;
        let mut frames = screen.frames(source, config.cursor)?;
        let constraints = screen.negotiate(&mut frames)?;
        let source_size = (constraints.width, constraints.height);
        let whole = Region::new(0, 0, source_size.0, source_size.1);
        let region = config.region.unwrap_or(whole);
        if !region.fits(source_size.0, source_size.1) {
            return Err(rikka_capture::Error::Region {
                x: region.x,
                y: region.y,
                width: region.width,
                height: region.height,
                source_width: source_size.0,
                source_height: source_size.1,
            }
            .into());
        }
        let area = Region::new(region.x, region.y, region.width & !1, region.height & !1);
        if area.width == 0 || area.height == 0 {
            return Err(Error::TooSmall);
        }
        let audio = config.audio.as_ref().map(open_audio).transpose()?;
        let mut encoding = EncoderConfig::new(area.width, area.height)
            .frame_rate(config.fps.max(1))
            .codec(config.codec)
            .backend(config.backend)
            .rate(config.rate)
            .speed(config.speed);
        if let Some(capture) = &audio {
            let mut track = AudioConfig::new(capture.sample_rate(), capture.channels());
            if let Some(bitrate) = config.audio.as_ref().and_then(|track| track.bitrate) {
                track = track.bitrate(bitrate);
            }
            encoding = encoding.audio(track);
        }
        let (picked, feed) = match pick_gpu(&mut screen, &constraints, path, encoding, area) {
            Ok(picked) => (picked, Feed::Dmabuf),
            Err(error) if config.backend == EncoderChoice::Only(EncoderBackend::VulkanVideo) => {
                return Err(error);
            }
            Err(error) => {
                tracing::info!("recording from memory: {error}");
                let picked = Picked {
                    encoder: VideoEncoder::new(None, path, encoding.feed(Feed::Memory))?,
                    pool: BufferPool::shm(&mut screen, &constraints, BUFFERS)?,
                    gpu: None,
                };
                (picked, Feed::Memory)
            }
        };
        Ok(Self {
            screen,
            frames,
            commands,
            encoder: picked.encoder,
            pool: picked.pool,
            gpu: picked.gpu,
            samples: vec![0.0; audio.as_ref().map_or(0, block)],
            audio,
            source_size,
            area,
            feed,
            path: path.to_path_buf(),
            started: Instant::now(),
        })
    }

    fn run(mut self, frames: &AtomicU64) -> Result<RecordSummary> {
        loop {
            if self.commands.drain().any(|Command::Stop| true) {
                break;
            }
            if self.frames.is_stopped() {
                break;
            }
            if !self.reallocate()? {
                tracing::info!("recorded source resized, stopping recording");
                break;
            }
            let due = self.started + self.encoder.next_frame_at();
            let now = Instant::now();
            if now >= due {
                self.pool.capture(&mut self.frames)?;
            }
            self.screen.flush()?;
            let drain = self.audio.is_some().then_some(AUDIO_DRAIN);
            let next = (!self.pool.is_pending()).then(|| due.saturating_duration_since(now));
            let wake = drain.into_iter().chain(next).min();
            let commands = &self.commands;
            self.screen
                .dispatch_with(|fd| commands.wait_readable(fd, wake))?;
            let mut ended = false;
            for captured in self.frames.collect(Some(&mut self.pool)) {
                match captured {
                    Captured::Ready { index, .. } => {
                        self.encode(index)?;
                        frames.store(self.encoder.frames(), Ordering::Relaxed);
                    }
                    Captured::Lost => {}
                    Captured::Ended => ended = true,
                }
            }
            self.drain_audio()?;
            if ended {
                break;
            }
        }
        self.drain_audio()?;
        let Session { encoder, path, .. } = self;
        let encoded = encoder.finish()?;
        Ok(RecordSummary {
            path,
            frames: encoded.frames,
            duration: encoded.duration,
            audio: encoded.audio,
        })
    }

    fn reallocate(&mut self) -> Result<bool> {
        let constraints = self.frames.constraints();
        if constraints.is_empty() || constraints.generation == self.pool.generation() {
            return Ok(true);
        }
        if (constraints.width, constraints.height) != self.source_size {
            return Ok(false);
        }
        self.pool = match self.feed {
            Feed::Dmabuf => {
                let gpu = self.gpu.as_ref().ok_or(Error::ThreadDied)?;
                let mut pool = BufferPool::dmabuf(&mut self.screen, gpu, &constraints, BUFFERS)?;
                let (encoder, area) = (&self.encoder, self.area);
                pool.bind(|spec| encoder.import(spec).map(|imported| crop(imported, area)))?;
                pool
            }
            Feed::Memory => BufferPool::shm(&mut self.screen, &constraints, BUFFERS)?,
        };
        Ok(true)
    }

    fn encode(&mut self, index: usize) -> Result<()> {
        let at = self.started.elapsed();
        match self.feed {
            Feed::Dmabuf => {
                if let Some(imported) = self.pool.imported(index) {
                    self.encoder.push_imported_at(imported, at)?;
                }
            }
            Feed::Memory => {
                let buffer = self.pool.buffer(index);
                let Some(pixels) = buffer.pixels() else {
                    return Ok(());
                };
                let stride = buffer.stride() as usize;
                let start = self.area.y as usize * stride + self.area.x as usize * 4;
                let picture = Picture {
                    bytes: &pixels[start..],
                    width: self.area.width,
                    height: self.area.height,
                    stride,
                    layout: match buffer.format().is_bgr() {
                        true => PixelLayout::Bgra,
                        false => PixelLayout::Rgba,
                    },
                };
                self.encoder.push_frame_at(picture, at)?;
            }
        }
        Ok(())
    }

    fn drain_audio(&mut self) -> Result<()> {
        let Some(capture) = &self.audio else {
            return Ok(());
        };
        loop {
            let read = capture.read(&mut self.samples);
            if read == 0 {
                return Ok(());
            }
            self.encoder.push_audio(&self.samples[..read])?;
            if read < self.samples.len() {
                return Ok(());
            }
        }
    }
}

fn pick_gpu(
    screen: &mut Screen,
    constraints: &Constraints,
    path: &Path,
    encoding: EncoderConfig,
    area: Region,
) -> Result<Picked> {
    let device = constraints
        .dmabuf
        .as_ref()
        .ok_or(rikka_capture::Error::NoSharedBuffer)?
        .device;
    let gpu = Gpu::builder()
        .app_name(THREAD)
        .drm_node(rustix::fs::major(device), rustix::fs::minor(device))
        .compute_support()
        .video_encode()
        .build()
        .map_err(rikka_video::Error::Gpu)?;
    let mut pool = BufferPool::dmabuf(screen, &gpu, constraints, BUFFERS)?;
    let encoder = VideoEncoder::new(Some(&gpu), path, encoding.feed(Feed::Dmabuf))?;
    pool.bind(|spec| encoder.import(spec).map(|imported| crop(imported, area)))?;
    Ok(Picked {
        encoder,
        pool,
        gpu: Some(gpu),
    })
}

fn crop(imported: Imported, area: Region) -> Imported {
    imported.cropped(
        (area.x.max(0) as u32, area.y.max(0) as u32),
        (area.width, area.height),
    )
}

fn block(capture: &AudioCapture) -> usize {
    let frames = u64::from(capture.sample_rate()) * AUDIO_DRAIN.as_millis() as u64 / 1000;
    frames as usize * usize::from(capture.channels())
}

fn open_audio(track: &AudioTrack) -> Result<AudioCapture> {
    let preferred = CaptureConfig::default()
        .source(track.source.clone())
        .sample_rate(PREFERRED_RATE)
        .channels(PREFERRED_CHANNELS)
        .buffered(AUDIO_HELD);
    match AudioCapture::start(preferred) {
        Ok(capture) => Ok(capture),
        Err(error) => {
            tracing::debug!(%error, "capturing sound in the device's own format");
            Ok(AudioCapture::start(
                CaptureConfig::default()
                    .source(track.source.clone())
                    .buffered(AUDIO_HELD),
            )?)
        }
    }
}
