use std::path::Path;
use std::time::Duration;

use rikka_image::{Frame, GifWriter, Loops};
use rikka_types::Flow;
use rikka_video::{
    EncodeSpeed, PixelSize, VideoCodec, VideoInfo, VideoTarget, convert, probe, sample,
};

use crate::error::ConvertError;
use crate::progress::Watch;
use crate::sound::audio_target;
use crate::target::{MovieFormat, SoundFormat};
use crate::tuning::{Scale, Tuning};

const ANIMATION_STEP: Duration = Duration::from_millis(100);

pub(crate) fn convert_movie(
    source: &Path,
    format: MovieFormat,
    tuning: Tuning,
    draft: &Path,
    watch: &Watch,
) -> Result<(), ConvertError> {
    let (video, sound) = match format {
        MovieFormat::Gif => return animate(source, tuning.scale, draft, watch),
        MovieFormat::Mp4 | MovieFormat::Mkv => (VideoCodec::H264, SoundFormat::Aac),
        MovieFormat::WebM => (VideoCodec::Vp9, SoundFormat::Opus),
    };
    let mut target = VideoTarget::new(video)
        .quality(tuning.quality.pick(65, 78, 90))
        .speed(tuning.quality.pick(
            EncodeSpeed::Balanced,
            EncodeSpeed::Balanced,
            EncodeSpeed::Slow,
        ));
    if let Some(side) = bound(tuning.scale, || Ok(probe(source)?))? {
        target = target.with_max_size(PixelSize::new(side, side));
    }
    let mut converting = convert(source, draft).video(target).subtitles();
    if let Some(audio) = audio_target(sound, tuning.quality) {
        converting = converting.audio(audio);
    }
    let watch = watch.clone();
    converting
        .on_progress(move |fraction| watch.reached(fraction))
        .run()?;
    Ok(())
}

fn bound(
    scale: Scale,
    measure: impl FnOnce() -> Result<VideoInfo, ConvertError>,
) -> Result<Option<u32>, ConvertError> {
    Ok(match scale {
        Scale::Original => None,
        Scale::LongestSide(side) => Some(side),
        relative => {
            let info = measure()?;
            let (width, height) = relative.fit(info.width, info.height);
            Some(width.max(height))
        }
    })
}

fn animate(source: &Path, scale: Scale, draft: &Path, watch: &Watch) -> Result<(), ConvertError> {
    let info = probe(source)?;
    let length = info.duration.unwrap_or_default().as_secs_f32();
    let mut sampling = sample(source).every(ANIMATION_STEP);
    if let Some(side) = bound(scale, || Ok(info))? {
        sampling = sampling.with_max_size(PixelSize::new(side, side));
    }
    let mut writer: Option<GifWriter> = None;
    let mut failed = None;
    let sampled = sampling.each(|at, picture| {
        let pushed = match writer.as_mut() {
            Some(writer) => writer.push(Frame::new(picture, ANIMATION_STEP)),
            None => {
                let (width, height) = picture.dimensions();
                GifWriter::create(draft, width, height, Loops::Infinite).and_then(|created| {
                    writer
                        .insert(created)
                        .push(Frame::new(picture, ANIMATION_STEP))
                })
            }
        };
        if let Err(refused) = pushed {
            failed = Some(refused);
            return Flow::Cancel;
        }
        let fraction = if length > 0.0 {
            at.as_secs_f32() / length
        } else {
            0.0
        };
        watch.reached(fraction)
    });
    if let Some(refused) = failed {
        return Err(refused.into());
    }
    sampled?;
    writer
        .ok_or_else(|| ConvertError::Other(format!("{} shows no pictures", source.display())))?
        .finish()?;
    Ok(())
}
