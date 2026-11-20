use std::path::Path;
use std::sync::Arc;

use cut_project::Project;
use rikka_audio_decode::WavWriter;
use rikka_types::CancelToken;
use rikka_video::{AudioConfig, AudioEncoder};

use super::{KILO, Progress, Report, Reporter, Summary};
use crate::error::Result;
use crate::mixdown::{CHANNELS, Mixdown};
use crate::output::OutputFile;
use crate::plan::Plan;
use crate::settings::{Audio, SoundCodec};
use crate::span::Span;

enum AudioSink {
    Wav(WavWriter),
    Aac(AudioEncoder),
}

impl AudioSink {
    fn open(audio: &Audio, path: &Path) -> Result<AudioSink> {
        Ok(match audio.codec {
            SoundCodec::Wav => AudioSink::Wav(WavWriter::create(
                path,
                audio.sample_rate,
                CHANNELS,
                audio.depth,
            )?),
            SoundCodec::Aac => AudioSink::Aac(AudioEncoder::new(
                path,
                AudioConfig::new(audio.sample_rate, CHANNELS)
                    .bitrate(u64::from(audio.bitrate) * KILO),
            )?),
        })
    }

    fn write(&mut self, samples: &[f32]) -> Result<()> {
        match self {
            AudioSink::Wav(writer) => writer.write(samples)?,
            AudioSink::Aac(encoder) => encoder.push(samples)?,
        }
        Ok(())
    }

    fn finish(self) -> Result<()> {
        match self {
            AudioSink::Wav(writer) => writer.finish()?,
            AudioSink::Aac(encoder) => {
                encoder.finish()?;
            }
        }
        Ok(())
    }
}

pub(super) fn tape(
    project: &Arc<Project>,
    span: Span,
    audio: &Audio,
    output: OutputFile,
    report: &Reporter,
    cancel: &CancelToken,
) -> Result<Option<Summary>> {
    let plan = Plan::new(span, project.rate(), project.rate());
    let mut sink = AudioSink::open(audio, output.draft())?;
    report(Report::Opened { backend: None });
    let mut mixdown = Mixdown::new(Arc::clone(project), span.first(), audio.sample_rate);
    let total = plan.frames();
    let mut progress = Progress::new(report, total);
    let mut samples = Vec::new();
    for index in 0..total {
        if cancel.is_cancelled() {
            return Ok(None);
        }
        let range = plan.samples(index, audio.sample_rate);
        samples.resize(
            (range.end - range.start) as usize * usize::from(CHANNELS),
            0.0,
        );
        mixdown.fill(&mut samples);
        sink.write(&samples)?;
        progress.tick(index + 1);
    }
    report(Report::Finishing);
    sink.finish()?;
    Ok(Some(Summary {
        path: output.commit()?,
        duration: plan.duration(),
        frames: total,
        backend: None,
        sound: true,
        undecodable: mixdown.undecodable().len(),
        offline: 0,
    }))
}
