use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::{Duration, Instant};

use cut_project::{Project, Rate};
use cut_render::{Request, Sequence};
use rikka_image_edit::{Imaging, Picture};
use rikka_types::CancelToken;
use rikka_video::{AudioConfig, EncodedVideo, EncoderConfig, RateControl, VideoEncoder};

use super::{KILO, Progress, Report, Reporter, Summary};
use crate::error::{Error, Result};
use crate::mixdown::{CHANNELS, Mixdown};
use crate::output::OutputFile;
use crate::plan::Plan;
use crate::settings::{Compression, Video};
use crate::span::Span;

pub(crate) const SAMPLE_RATE: u32 = 48_000;
const AHEAD: usize = 3;
const MEGA: u64 = 1_000_000;
const PREVIEW_EVERY: Duration = Duration::from_millis(400);

pub(super) struct Take<'a> {
    pub(super) project: &'a Arc<Project>,
    pub(super) span: Span,
    pub(super) video: &'a Video,
    pub(super) imaging: &'a Imaging,
}

type Pixels = Arc<Vec<u8>>;

struct EncodedFilm {
    encoded: EncodedVideo,
    undecodable: usize,
}

pub(super) fn film(
    take: &Take<'_>,
    output: OutputFile,
    report: &Reporter,
    cancel: &CancelToken,
) -> Result<Option<Summary>> {
    let project = take.project;
    let size = take.video.resolution.size(project.format());
    let plan = Plan::new(
        take.span,
        project.rate(),
        take.video.frame_rate.rate(project.rate()),
    );
    let sound = take.video.sound && project.is_audible();
    let encoder = VideoEncoder::new(
        None,
        output.draft(),
        config(take.video, size, plan.output(), sound),
    )?;
    let backend = encoder.backend();
    report(Report::Opened {
        backend: Some(backend),
    });
    let press = Press {
        encoder,
        mixdown: sound.then(|| Mixdown::new(Arc::clone(project), take.span.first(), SAMPLE_RATE)),
        plan,
        report: Arc::clone(report),
    };
    let (frames, taken) = sync_channel::<Pixels>(AHEAD);
    let pressing = std::thread::Builder::new()
        .name("rikka-cut-encode".to_owned())
        .spawn(move || press.run(&taken))
        .map_err(|error| Error::Spawn(rikka_worker::Error::Spawn(error)))?;
    let drawn = draw(take, &plan, size, &frames, report, cancel);
    drop(frames);
    let pressed = pressing.join().unwrap_or(Err(Error::Encoder));
    let (offline, pressed) = match (drawn, pressed) {
        (Ok(None), _) => return Ok(None),
        (_, Err(trouble)) | (Err(trouble), _) => return Err(trouble),
        (Ok(Some(_)), Ok(None)) => return Err(Error::Encoder),
        (Ok(Some(offline)), Ok(Some(pressed))) => (offline, pressed),
    };
    Ok(Some(Summary {
        path: output.commit()?,
        duration: pressed.encoded.duration,
        frames: pressed.encoded.frames,
        backend: Some(backend),
        sound: pressed.encoded.audio,
        undecodable: pressed.undecodable,
        offline,
    }))
}

fn config(video: &Video, (width, height): (u32, u32), rate: Rate, sound: bool) -> EncoderConfig {
    let control = match video.compression {
        Compression::Quality => RateControl::Quality(video.quality),
        Compression::Bitrate => RateControl::Bitrate(u64::from(video.bitrate) * MEGA),
    };
    let config = EncoderConfig::new(width, height)
        .frame_rate_rational(rate.numerator(), rate.denominator())
        .codec(video.codec)
        .backend(video.encoder)
        .rate(control)
        .speed(video.speed)
        .fast_start();
    match sound {
        true => config.audio(
            AudioConfig::new(SAMPLE_RATE, CHANNELS).bitrate(u64::from(video.sound_bitrate) * KILO),
        ),
        false => config,
    }
}

fn draw(
    take: &Take<'_>,
    plan: &Plan,
    (width, height): (u32, u32),
    frames: &SyncSender<Pixels>,
    report: &Reporter,
    cancel: &CancelToken,
) -> Result<Option<usize>> {
    let mut sequence = Sequence::new(take.imaging.clone(), cancel.clone());
    let mut offline = BTreeSet::new();
    let mut shown: Option<Instant> = None;
    let mut last: Option<(Picture, Pixels)> = None;
    for index in 0..plan.frames() {
        let request = Request::new(Arc::clone(take.project), plan.frame(index))
            .sized(width, height)
            .rolling();
        let Some(composite) = sequence.render(&request)? else {
            return Ok(None);
        };
        offline.extend(composite.offline);
        let pixels = match &last {
            Some((picture, pixels)) if *picture == composite.picture => Arc::clone(pixels),
            _ => Arc::new(composite.picture.to_rgba8()?),
        };
        if shown.is_none_or(|shown| shown.elapsed() >= PREVIEW_EVERY) {
            shown = Some(Instant::now());
            report(Report::Previewed(composite.picture.shown()?));
        }
        last = Some((composite.picture, Arc::clone(&pixels)));
        if frames.send(pixels).is_err() {
            return Err(Error::Encoder);
        }
    }
    Ok(Some(offline.len()))
}

struct Press {
    encoder: VideoEncoder,
    mixdown: Option<Mixdown>,
    plan: Plan,
    report: Reporter,
}

impl Press {
    fn run(mut self, taken: &Receiver<Pixels>) -> Result<Option<EncodedFilm>> {
        let total = self.plan.frames();
        let mut progress = Progress::new(&self.report, total);
        let mut pushed = 0;
        let mut samples = Vec::new();
        for pixels in taken {
            if let Some(mixdown) = &mut self.mixdown {
                let range = self.plan.samples(pushed, mixdown.sample_rate());
                samples.resize(
                    (range.end - range.start) as usize * usize::from(CHANNELS),
                    0.0,
                );
                mixdown.fill(&mut samples);
                self.encoder.push_audio(&samples)?;
            }
            self.encoder.push_frame(&pixels)?;
            pushed += 1;
            progress.tick(pushed);
        }
        if pushed < total {
            return Ok(None);
        }
        (self.report)(Report::Finishing);
        let undecodable = self
            .mixdown
            .as_ref()
            .map_or(0, |mixdown| mixdown.undecodable().len());
        Ok(Some(EncodedFilm {
            encoded: self.encoder.finish()?,
            undecodable,
        }))
    }
}
