use std::ops::ControlFlow;
#[cfg(unix)]
use std::os::fd::AsFd;
#[cfg(windows)]
use std::os::windows::io::AsHandle;
use std::time::{Duration, Instant};

use rikka_opus::{OPUS_MAX_PACKET, OPUS_SAMPLE_RATE, OpusApplication, OpusEncoder, OpusSignal};
use rikka_worker::{CommandReceiver, CommandSender, Worker, command_channel};

use crate::capture::{AudioCapture, Fill};
use crate::error::{Error, Result};
use crate::spawn;

const STALL: Duration = Duration::from_millis(100);
const QUIET_LEVEL: f32 = 1.0e-4;
const QUIET_SPAN: Duration = Duration::from_millis(500);
const BITRATE: u32 = 128_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub struct SoundEncoding {
    thread: &'static str,
    application: OpusApplication,
    frame: usize,
    bitrate: u32,
}

impl SoundEncoding {
    pub fn new(thread: &'static str, application: OpusApplication, frame: usize) -> Self {
        SoundEncoding {
            thread,
            application,
            frame: frame.max(1),
            bitrate: BITRATE,
        }
    }

    pub fn bitrate(mut self, bits_per_second: u32) -> Self {
        self.bitrate = bits_per_second;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sent<'a> {
    Packet { data: &'a [u8], at: u64 },
    Quiet,
}

enum Command {
    Stop,
}

#[derive(Debug)]
pub struct SoundSender {
    _worker: Worker<CommandSender<Command>>,
}

impl SoundSender {
    pub fn start(
        capture: AudioCapture,
        encoding: SoundEncoding,
        sink: impl FnMut(Sent<'_>) -> ControlFlow<()> + Send + 'static,
    ) -> Result<SoundSender> {
        if capture.sample_rate() != OPUS_SAMPLE_RATE {
            return Err(Error::OpusRate(capture.sample_rate()));
        }
        let mut encoder = OpusEncoder::new(encoding.application, capture.channels())?;
        encoder.set_signal(OpusSignal::Music)?;
        encoder.set_bitrate(encoding.bitrate)?;
        let (commands, received) = command_channel(|| Command::Stop)?;
        let worker = Worker::spawn(encoding.thread, commands, move || {
            Ok(move || Sending::new(capture, encoder, encoding.frame).run(&received, sink))
        })
        .map_err(spawn::failed(encoding.thread))?;
        Ok(SoundSender { _worker: worker })
    }
}

struct Sending {
    capture: AudioCapture,
    encoder: OpusEncoder,
    frame: Vec<f32>,
    frames: u64,
    packet: Vec<u8>,
    started: Instant,
    at: u64,
    flowing: bool,
    sounding: bool,
    quiet: u64,
    quiet_limit: u64,
}

impl Sending {
    fn new(capture: AudioCapture, encoder: OpusEncoder, frames: usize) -> Sending {
        let channels = usize::from(capture.channels());
        let frame_span = frames as f64 / f64::from(OPUS_SAMPLE_RATE);
        Sending {
            frame: vec![0.0; frames * channels],
            frames: frames as u64,
            packet: vec![0; OPUS_MAX_PACKET],
            started: Instant::now(),
            at: 0,
            flowing: false,
            sounding: false,
            quiet: 0,
            quiet_limit: (QUIET_SPAN.as_secs_f64() / frame_span).ceil() as u64,
            capture,
            encoder,
        }
    }

    fn run(
        mut self,
        commands: &CommandReceiver<Command>,
        mut sink: impl FnMut(Sent<'_>) -> ControlFlow<()>,
    ) {
        let mut stalls_at = Some(Instant::now() + STALL);
        loop {
            if commands.drain().any(|Command::Stop| true) {
                break;
            }
            match self.capture.fill(&mut self.frame, Duration::ZERO) {
                Fill::Filled => {
                    stalls_at = Some(Instant::now() + STALL);
                    if self.send(&mut sink).is_break() {
                        return;
                    }
                    continue;
                }
                Fill::Ended => {
                    tracing::info!("the captured sound ended");
                    break;
                }
                Fill::Silent => {}
            }
            if stalls_at.is_some_and(|at| Instant::now() >= at) {
                stalls_at = None;
                self.flowing = false;
                if self.hush(&mut sink).is_break() {
                    return;
                }
            }
            let timeout = stalls_at.map(|at| at.saturating_duration_since(Instant::now()));
            if let Err(error) = wait_for(commands, &self.capture, timeout) {
                tracing::warn!("cannot wait for captured sound: {error}");
                break;
            }
        }
        let _ = self.hush(&mut sink);
    }

    fn send(&mut self, sink: &mut impl FnMut(Sent<'_>) -> ControlFlow<()>) -> ControlFlow<()> {
        if !self.flowing {
            let elapsed = self.started.elapsed().as_secs_f64() * f64::from(OPUS_SAMPLE_RATE);
            let resumed = (elapsed as u64) / self.frames * self.frames;
            self.at = self.at.max(resumed);
            self.flowing = true;
        }
        let at = self.at;
        self.at += self.frames;
        let loud = self.frame.iter().any(|sample| sample.abs() > QUIET_LEVEL);
        self.quiet = if loud { 0 } else { self.quiet + 1 };
        if self.quiet > self.quiet_limit {
            return self.hush(sink);
        }
        self.sounding = true;
        match self.encoder.encode(&self.frame, &mut self.packet) {
            Ok(length) => sink(Sent::Packet {
                data: &self.packet[..length],
                at,
            }),
            Err(error) => {
                tracing::debug!("failed to encode captured sound: {error}");
                ControlFlow::Continue(())
            }
        }
    }

    fn hush(&mut self, sink: &mut impl FnMut(Sent<'_>) -> ControlFlow<()>) -> ControlFlow<()> {
        match std::mem::take(&mut self.sounding) {
            true => sink(Sent::Quiet),
            false => ControlFlow::Continue(()),
        }
    }
}

#[cfg(unix)]
fn wait_for(
    commands: &CommandReceiver<Command>,
    capture: &AudioCapture,
    timeout: Option<Duration>,
) -> std::io::Result<bool> {
    commands.wait_readable(capture.as_fd(), timeout)
}

#[cfg(windows)]
fn wait_for(
    commands: &CommandReceiver<Command>,
    capture: &AudioCapture,
    timeout: Option<Duration>,
) -> std::io::Result<bool> {
    commands.wait_signaled(capture.as_handle(), timeout)
}
