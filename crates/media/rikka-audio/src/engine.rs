use rikka_sync::Mutex;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::buffered::BufferedSource;
use crate::config::{EngineConfig, RESAMPLE_QUALITY};
use crate::error::{Error, Result, Role};
use crate::feed::AudioFeed;
use crate::frame::check_channels;
use crate::handle::PlaybackHandle;
use crate::host::{Shared, StreamHost};
use crate::lost::DeviceLost;
use crate::mixer::{Mixer, Retired};
use crate::output::{OutputDevice, OutputPlan};
use crate::param::Param;
use crate::play_params::PlayParams;
use crate::stream::StreamSource;
use crate::suspend::Activity;
use crate::time::{duration_to_frames, frames_to_duration};
use crate::voice::{Voice, VoiceShared, VoiceSource};
use rikka_audio_decode::{Decoder, Sound};

const COMMAND_QUEUE_CAPACITY: usize = 1024;
const RETIRED_QUEUE_CAPACITY: usize = 4096;

pub struct AudioEngine {
    commands: Mutex<rikka_ring::Producer<Box<Voice>>>,
    master_volume: Param,
    device_lost: Arc<DeviceLost>,
    host: StreamHost,
    sequence: AtomicU64,
    output_rate: u32,
    latency: Arc<AtomicU64>,
}

impl AudioEngine {
    pub fn devices() -> Result<Vec<OutputDevice>> {
        crate::backend::output_devices()
    }
    pub fn new(config: EngineConfig) -> Result<AudioEngine> {
        let (command_tx, command_rx) = rikka_ring::channel(COMMAND_QUEUE_CAPACITY);
        let (retired_tx, retired_rx) = rikka_ring::channel::<Retired>(RETIRED_QUEUE_CAPACITY);
        let master_volume = Param::new(1.0);
        let device_lost = Arc::new(DeviceLost::default());
        let activity = Arc::new(Activity::default());
        let latency = Arc::new(AtomicU64::new(0));

        let volume = master_volume.clone();
        let heard = Arc::clone(&activity);
        let mix = move |plan: &OutputPlan| {
            Mixer::new(
                plan.rate,
                plan.channels,
                command_rx,
                retired_tx,
                volume,
                heard,
            )
        };
        let (host, plan) = StreamHost::start(
            config.clone(),
            mix,
            Shared {
                watched: Arc::default(),
                activity,
                device_lost: Arc::clone(&device_lost),
                latency: Arc::clone(&latency),
            },
            retired_rx,
        )?;
        Ok(AudioEngine {
            commands: Mutex::new(command_tx),
            master_volume,
            device_lost,
            host,
            sequence: AtomicU64::new(0),
            output_rate: plan.rate,
            latency,
        })
    }

    pub fn output_sample_rate(&self) -> u32 {
        self.output_rate
    }

    #[must_use]
    pub fn output_latency(&self) -> Duration {
        frames_to_duration(self.latency.load(Ordering::Relaxed), self.output_rate)
    }

    pub fn is_device_lost(&self) -> bool {
        self.device_lost.is_lost()
    }

    pub fn load<'a>(&self, source: impl Into<rikka_types::Source<'a>>) -> Result<Sound> {
        Ok(rikka_audio_decode::read(source)
            .decode()?
            .resampled(self.output_rate, RESAMPLE_QUALITY))
    }

    pub fn play(&self, sound: &Sound) -> Result<PlaybackHandle> {
        self.play_with(sound, PlayParams::new())
    }

    pub fn play_with(&self, sound: &Sound, params: PlayParams) -> Result<PlaybackHandle> {
        check_channels(sound.channels(), Role::Source)?;
        let source_rate = sound.sample_rate().max(1);
        let to_frame = |duration| duration_to_frames(duration, source_rate);
        let frames = sound.frames();
        let start = params.start_at.map_or(0, to_frame);

        let shared = Arc::new(VoiceShared::from(&params));
        shared
            .position_frames
            .store(start.min(frames), Ordering::Relaxed);

        let source = BufferedSource::new(sound.clone(), start);
        self.start_voice(shared.clone(), VoiceSource::Buffered(source), source_rate)?;

        Ok(PlaybackHandle {
            shared,
            stream: None,
            fed: false,
            source_rate,
            waker: self.host.waker().clone(),
            watched: self.host.watched(),
            duration: Some(sound.duration()),
        })
    }

    pub fn stream(&self, path: impl AsRef<Path>) -> Result<PlaybackHandle> {
        self.stream_with(path, PlayParams::new())
    }

    pub fn stream_with(
        &self,
        path: impl AsRef<Path>,
        params: PlayParams,
    ) -> Result<PlaybackHandle> {
        self.stream_decoder(rikka_audio_decode::read(path.as_ref()).decoder()?, params)
    }

    fn stream_decoder(&self, decoder: Decoder, params: PlayParams) -> Result<PlaybackHandle> {
        let shared = Arc::new(VoiceShared::from(&params));
        let source = StreamSource::open(
            decoder,
            &params,
            shared.clone(),
            self.output_rate,
            RESAMPLE_QUALITY,
        )?;
        let stream_shared = source.shared().clone();
        let duration = stream_shared.duration();

        self.start_voice(
            shared.clone(),
            VoiceSource::Stream(Box::new(source)),
            self.output_rate,
        )?;

        Ok(PlaybackHandle {
            shared,
            stream: Some(stream_shared),
            fed: false,
            source_rate: self.output_rate,
            waker: self.host.waker().clone(),
            watched: self.host.watched(),
            duration,
        })
    }

    pub fn feed(&self, sample_rate: u32, channels: u16) -> Result<(PlaybackHandle, AudioFeed)> {
        let params = PlayParams::new();
        let shared = Arc::new(VoiceShared::from(&params));
        let fed = AudioFeed::open(
            sample_rate,
            channels,
            shared.clone(),
            self.output_rate,
            RESAMPLE_QUALITY,
        )?;
        self.device_lost.wake_on_loss(&fed.shared);
        self.start_voice(
            shared.clone(),
            VoiceSource::Stream(Box::new(fed.source)),
            self.output_rate,
        )?;

        let handle = PlaybackHandle {
            shared,
            stream: Some(fed.shared),
            fed: true,
            source_rate: self.output_rate,
            waker: self.host.waker().clone(),
            watched: self.host.watched(),
            duration: None,
        };
        Ok((handle, fed.feed))
    }

    pub fn set_master_volume(&self, volume: f32) {
        self.master_volume.set(volume.max(0.0));
    }

    fn start_voice(
        &self,
        shared: Arc<VoiceShared>,
        source: VoiceSource,
        source_rate: u32,
    ) -> Result<()> {
        let voice = Voice::new(
            shared,
            source,
            source_rate,
            self.sequence.fetch_add(1, Ordering::Relaxed),
        );
        let mut commands = self.commands.lock();
        commands
            .push(Box::new(voice))
            .map_err(|_| Error::CommandQueueFull)?;
        self.host.waker().wake();
        Ok(())
    }
}

impl std::fmt::Debug for AudioEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioEngine")
            .field("output_rate", &self.output_rate)
            .field("latency", &self.output_latency())
            .finish_non_exhaustive()
    }
}
