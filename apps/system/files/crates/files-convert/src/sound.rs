use std::path::Path;

use rikka_audio_decode::{Decoder, WavDepth, WavWriter};
use rikka_types::Flow;
use rikka_video::{
    AudioCodec, AudioConfig, AudioEncoder, AudioTarget, EncodeSpeed, RateControl, convert,
};

use crate::error::ConvertError;
use crate::progress::Watch;
use crate::tags;
use crate::target::SoundFormat;
use crate::tuning::Quality;

const DEEPEST: u32 = 24;
const SHALLOWEST: u32 = 16;

pub(crate) fn convert_sound(
    source: &Path,
    format: SoundFormat,
    quality: Quality,
    draft: &Path,
    watch: &Watch,
) -> Result<(), ConvertError> {
    write_decoded(source, format, quality, draft, watch)?;
    tags::copy_tags(source, draft);
    Ok(())
}

pub(crate) fn extract(
    source: &Path,
    format: SoundFormat,
    quality: Quality,
    draft: &Path,
    watch: &Watch,
) -> Result<(), ConvertError> {
    let Some(target) = audio_target(format, quality) else {
        return write_decoded(source, format, quality, draft, watch);
    };
    let watch = watch.clone();
    convert(source, draft)
        .audio(target)
        .on_progress(move |fraction| watch.reached(fraction))
        .run()?;
    Ok(())
}

pub(crate) fn audio_target(format: SoundFormat, quality: Quality) -> Option<AudioTarget> {
    let codec = codec(format)?;
    let target = match rate_control(format, quality) {
        RateControl::Bitrate(bits) => AudioTarget::new(codec).bitrate(bits),
        RateControl::Quality(level) => AudioTarget::new(codec).quality(level),
        RateControl::Default => AudioTarget::new(codec),
    };
    Some(target.speed(speed(quality)))
}

fn write_decoded(
    source: &Path,
    format: SoundFormat,
    quality: Quality,
    draft: &Path,
    watch: &Watch,
) -> Result<(), ConvertError> {
    let mut decoder = rikka_audio_decode::read(source).decoder()?;
    let mut writer = Writer::open(&decoder, format, quality, draft)?;
    let channels = usize::from(decoder.channels().max(1));
    let total = decoder.frames().filter(|frames| *frames > 0);
    let mut done = 0u64;
    let mut samples = Vec::new();
    loop {
        samples.clear();
        if !decoder.read(&mut samples)? {
            break;
        }
        writer.push(&samples)?;
        done += (samples.len() / channels) as u64;
        let fraction = total.map_or(0.0, |total| done as f32 / total as f32);
        if watch.reached(fraction) == Flow::Cancel {
            return Err(ConvertError::Cancelled);
        }
    }
    writer.finish()
}

enum Writer {
    Encoded(AudioEncoder),
    Wav(WavWriter),
}

impl Writer {
    fn open(
        decoder: &Decoder,
        format: SoundFormat,
        quality: Quality,
        draft: &Path,
    ) -> Result<Writer, ConvertError> {
        let (rate, channels) = (decoder.sample_rate(), decoder.channels());
        let bits = decoder.info().bits_per_sample;
        let Some(codec) = codec(format) else {
            let depth = match bits {
                Some(bits) if bits > SHALLOWEST => WavDepth::Pcm24,
                _ => WavDepth::Pcm16,
            };
            return Ok(Writer::Wav(WavWriter::create(
                draft, rate, channels, depth,
            )?));
        };
        let mut config = AudioConfig::new(rate, channels)
            .codec(codec)
            .rate(rate_control(format, quality))
            .speed(speed(quality));
        if codec.is_lossless() {
            config = config.bits_per_sample(bits.unwrap_or(SHALLOWEST).clamp(SHALLOWEST, DEEPEST));
        }
        Ok(Writer::Encoded(AudioEncoder::new(draft, config)?))
    }

    fn push(&mut self, samples: &[f32]) -> Result<(), ConvertError> {
        match self {
            Writer::Encoded(encoder) => encoder.push(samples)?,
            Writer::Wav(writer) => writer.write(samples)?,
        }
        Ok(())
    }

    fn finish(self) -> Result<(), ConvertError> {
        match self {
            Writer::Encoded(encoder) => {
                encoder.finish()?;
            }
            Writer::Wav(writer) => writer.finish()?,
        }
        Ok(())
    }
}

fn rate_control(format: SoundFormat, quality: Quality) -> RateControl {
    match format {
        SoundFormat::Mp3 => quality.pick(
            RateControl::Quality(78),
            RateControl::Quality(100),
            RateControl::Bitrate(320_000),
        ),
        SoundFormat::Aac => RateControl::Quality(quality.pick(33, 67, 100)),
        SoundFormat::Opus => RateControl::Quality(quality.pick(40, 60, 100)),
        SoundFormat::Vorbis => RateControl::Quality(quality.pick(50, 70, 90)),
        SoundFormat::Flac | SoundFormat::Alac | SoundFormat::Wav => RateControl::Default,
    }
}

fn speed(quality: Quality) -> EncodeSpeed {
    quality.pick(
        EncodeSpeed::Balanced,
        EncodeSpeed::Balanced,
        EncodeSpeed::Slow,
    )
}

fn codec(format: SoundFormat) -> Option<AudioCodec> {
    Some(match format {
        SoundFormat::Mp3 => AudioCodec::Mp3,
        SoundFormat::Aac => AudioCodec::Aac,
        SoundFormat::Opus => AudioCodec::Opus,
        SoundFormat::Vorbis => AudioCodec::Vorbis,
        SoundFormat::Flac => AudioCodec::Flac,
        SoundFormat::Alac => AudioCodec::Alac,
        SoundFormat::Wav => return None,
    })
}
