use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, channel, sync_channel};

use animate_stage::{Recorder, StageError};
use rikka_audio_decode::Sound;
use rikka_renderer::{Readback, Renderer};
use rikka_types::PixelSize;
use rikka_video::{AudioConfig, EncoderConfig, RateControl, VideoEncoder};

use super::encode::Encoder;
use super::plan::Plan;
use super::{ExportError, Report, Reporter, Request, Summary};
use crate::encoding::Rate;
use crate::export::Export;

const READBACKS: usize = 4;
const AHEAD: usize = 2;
const KILO: u64 = 1000;
const MEGA: u64 = 1_000_000;

enum Halt {
    Cancelled,
    Encoder,
    Failed(StageError),
}

pub(crate) fn run(request: Request, report: &Reporter, cancel: &Arc<AtomicBool>) {
    let path = request.path.clone();
    let ended = film(request, report, cancel);
    let last = match ended {
        Ok(Some(summary)) => Report::Finished(summary),
        Ok(None) => {
            discard(&path);
            Report::Cancelled
        }
        Err(error) => {
            discard(&path);
            Report::Failed(error)
        }
    };
    report(last);
}

fn discard(path: &std::path::Path) {
    if let Err(error) = std::fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(%error, path = %path.display(), "cannot remove partial export");
    }
}

fn film(
    request: Request,
    report: &Reporter,
    cancel: &Arc<AtomicBool>,
) -> Result<Option<Summary>, ExportError> {
    let Request {
        stage,
        export,
        sound,
        path,
    } = request;
    let size = PixelSize::new(export.resolution.width(), export.resolution.height());
    let plan = Plan::for_export(&export);
    let renderer = Renderer::new(
        &rikka_gpu::Gpu::builder()
            .app_name("rikka-animate")
            .with_3d()
            .background_compute()
            .build()
            .map_err(ExportError::Renderer)?,
    )
    .map_err(ExportError::Renderer)?;
    let mut recorder = Recorder::open(stage, renderer, size)?;
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    let sound = sound.filter(|_| export.encoding.sound);
    let encoder = VideoEncoder::new(
        Some(recorder.renderer().gpu()),
        &path,
        config(&export, sound.as_ref()),
    )?;
    let backend = encoder.backend();
    report(Report::Opened { backend });

    let (shots, receiver) = sync_channel::<Readback>(READBACKS);
    let (returned, free) = channel::<Readback>();
    for _ in 0..READBACKS {
        let _ = returned.send(recorder.readback());
    }
    let pressing = Encoder {
        encoder,
        plan,
        sound,
        size,
        report: Arc::clone(report),
        cancel: Arc::clone(cancel),
    };
    let encoder = std::thread::Builder::new()
        .name("rikka-animate-encode".to_owned())
        .spawn(move || pressing.run(&receiver, &returned))
        .map_err(ExportError::Thread)?;

    let rendered = render(&mut recorder, &plan, &shots, &free, cancel);
    drop(shots);
    let encoded = encoder.join().unwrap_or(Err(ExportError::EncoderPanicked));
    drop(free);
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }

    match (rendered, encoded) {
        (Err(Halt::Cancelled), _) | (Ok(()), Ok(None)) => Ok(None),
        (Err(Halt::Failed(error)), _) => Err(error.into()),
        (_, Err(error)) => Err(error),
        (Err(Halt::Encoder), Ok(_)) => Err(ExportError::EncoderStopped),
        (Ok(()), Ok(Some(encoded))) => Ok(Some(Summary {
            path,
            frames: encoded.frames,
            duration: encoded.duration,
            sound: encoded.audio,
            backend,
        })),
    }
}

fn render(
    recorder: &mut Recorder,
    plan: &Plan,
    shots: &SyncSender<Readback>,
    free: &Receiver<Readback>,
    cancel: &AtomicBool,
) -> Result<(), Halt> {
    let mut waiting = VecDeque::with_capacity(READBACKS);
    let deliver = |recorder: &mut Recorder, mut readback: Readback| {
        recorder.read_back(&mut readback).map_err(Halt::Failed)?;
        shots.send(readback).map_err(|_| Halt::Encoder)
    };
    for index in 0..plan.frames() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Halt::Cancelled);
        }
        let mut readback = loop {
            match free.try_recv() {
                Ok(readback) => break readback,
                Err(TryRecvError::Empty) => match waiting.pop_front() {
                    Some(oldest) => deliver(recorder, oldest)?,
                    None => break free.recv().map_err(|_| Halt::Encoder)?,
                },
                Err(TryRecvError::Disconnected) => return Err(Halt::Encoder),
            }
        };
        recorder
            .render_frame(plan.time(index), &mut readback)
            .map_err(Halt::Failed)?;
        waiting.push_back(readback);
        if waiting.len() > AHEAD
            && let Some(oldest) = waiting.pop_front()
        {
            deliver(recorder, oldest)?;
        }
    }
    while let Some(oldest) = waiting.pop_front() {
        deliver(recorder, oldest)?;
    }
    Ok(())
}

fn config(export: &Export, sound: Option<&Sound>) -> EncoderConfig {
    let encoding = export.encoding;
    let rate = match encoding.rate {
        Rate::Quality => RateControl::Quality(encoding.quality),
        Rate::Bitrate => RateControl::Bitrate(u64::from(encoding.bitrate) * MEGA),
    };
    let config = EncoderConfig::new(export.resolution.width(), export.resolution.height())
        .frame_rate_rational(
            encoding.frame_rate.numerator(),
            encoding.frame_rate.denominator(),
        )
        .codec(encoding.codec)
        .backend(encoding.encoder)
        .rate(rate)
        .speed(encoding.speed)
        .fast_start();
    match sound {
        Some(sound) => config.audio(
            AudioConfig::new(sound.sample_rate(), sound.channels())
                .bitrate(u64::from(encoding.sound_bitrate) * KILO),
        ),
        None => config,
    }
}
