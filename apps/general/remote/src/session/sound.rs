use std::convert::Infallible;
use std::time::{Duration, Instant};

use rikka_audio::{AudioEngine, AudioFeed, EngineConfig, PlaybackHandle};
use rikka_opus::OpusDecoder;
use rikka_playout::{CHANNELS, CHUNK, Packet, Playout, QueueTarget, RATE};
use rikka_remote_wire::{AUDIO_FRAME_SAMPLES, AUDIO_RATE};
use rikka_worker::{CommandReceiver, CommandSender, Stop, Worker, command_channel};

const THREAD: &str = "rikka-remote-viewer-sound";
const MAX_CHUNKS_PER_PULL: u32 = 8;
const CHUNK_SPAN: Duration = Duration::from_millis(CHUNK as u64 * 1000 / RATE as u64);

#[derive(Debug, thiserror::Error)]
pub(crate) enum SoundError {
    #[error(transparent)]
    Output(#[from] rikka_audio::Error),
    #[error(transparent)]
    Decoder(#[from] rikka_opus::Error),
    #[error("cannot start the sound thread: {0}")]
    Thread(#[from] std::io::Error),
    #[error("the sound thread stopped before it began")]
    Exited,
}

impl From<rikka_worker::Error<Infallible>> for SoundError {
    fn from(error: rikka_worker::Error<Infallible>) -> SoundError {
        match error {
            rikka_worker::Error::Spawn(error) => SoundError::Thread(error),
            rikka_worker::Error::Startup(never) => match never {},
            rikka_worker::Error::Exited => SoundError::Exited,
        }
    }
}

enum Heard {
    Packet(Packet),
    Stop,
}

struct Silencing {
    heard: CommandSender<Heard>,
    output: PlaybackHandle,
}

impl Stop for Silencing {
    fn stop(&self) {
        self.heard.stop();
        self.output.stop();
    }
}

pub(crate) struct SoundWorker {
    worker: Worker<Silencing>,
}

impl SoundWorker {
    pub(crate) fn start() -> Result<SoundWorker, SoundError> {
        let engine = AudioEngine::new(EngineConfig::new())?;
        let (output, feed) = engine.feed(AUDIO_RATE, CHANNELS as u16)?;
        let playout = Playout::new(OpusDecoder::new(CHANNELS as u16)?, Instant::now());
        let (heard, received) = command_channel(|| Heard::Stop)?;
        let silencing = Silencing {
            heard,
            output: output.clone(),
        };
        let worker = Worker::spawn(THREAD, silencing, move || {
            Ok::<_, Infallible>(move || {
                Playing {
                    _engine: engine,
                    output,
                    feed,
                    playout,
                    target: QueueTarget::new(),
                    chunk: [0.0; CHUNK * CHANNELS],
                }
                .run(&received);
            })
        })?;
        Ok(SoundWorker { worker })
    }

    pub(crate) fn push(&self, timestamp: u32, primary: &[u8], previous: Option<&[u8]>) {
        let arrived = Instant::now();
        let earlier = previous.map(|previous| Packet {
            timestamp: timestamp.wrapping_sub(AUDIO_FRAME_SAMPLES),
            arrived,
            payload: previous.to_vec(),
        });
        let latest = Packet {
            timestamp,
            arrived,
            payload: primary.to_vec(),
        };
        for packet in earlier.into_iter().chain([latest]) {
            if let Err(error) = self.worker.handle().heard.send(Heard::Packet(packet)) {
                tracing::debug!("the sound thread is gone: {error}");
                return;
            }
        }
    }
}

struct Playing {
    _engine: AudioEngine,
    output: PlaybackHandle,
    feed: AudioFeed,
    playout: Playout<OpusDecoder>,
    target: QueueTarget,
    chunk: [f32; CHUNK * CHANNELS],
}

impl Playing {
    fn run(mut self, heard: &CommandReceiver<Heard>) {
        loop {
            for item in heard.drain() {
                match item {
                    Heard::Packet(packet) => self.playout.push(packet),
                    Heard::Stop => return,
                }
            }
            if let Err(error) = self.play(Instant::now()) {
                tracing::warn!("the sound output stopped: {error}");
                return;
            }
            let queued = self.feed.queued();
            if queued >= self.target.target() && !self.feed.wait_until_queued_below(queued, None) {
                return;
            }
        }
    }

    fn play(&mut self, now: Instant) -> Result<(), rikka_audio::Error> {
        let mut queued = self.feed.queued();
        self.target.observe(now, queued, self.output.underruns());
        let mut produced = 0;
        while queued < self.target.target() && produced < MAX_CHUNKS_PER_PULL {
            self.playout.pull(now, &mut self.chunk);
            self.feed.write(&self.chunk)?;
            self.target.wrote(CHUNK_SPAN);
            produced += 1;
            queued = self.feed.queued();
        }
        Ok(())
    }
}
