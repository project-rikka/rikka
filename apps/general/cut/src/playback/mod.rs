mod preview;
mod voices;

use std::time::{Duration, Instant};

use cut_project::Alignment;
use cut_project::{Clip, Project, Stream};

use crate::live::LivePreview;

pub(crate) use preview::Preview;

use voices::{Cue, SourceLine, Voices};

const PREROLL: Duration = Duration::from_millis(1500);

#[derive(Debug, Default)]
pub(crate) struct Playback {
    voices: Voices,
    preview: Preview,
}

impl Playback {
    pub(crate) fn preview(&self) -> &Preview {
        &self.preview
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.preview.is_ready()
    }

    pub(crate) fn set_volume(&mut self, volume: f32) {
        self.voices.set_volume(volume);
    }

    pub(crate) fn halt(&mut self) {
        self.voices.silence();
    }

    pub(crate) fn reopen_offline(&mut self) {
        self.preview.reopen_offline();
        self.voices.forget_vanished();
    }

    pub(crate) fn listen(
        &mut self,
        project: &Project,
        live: &LivePreview,
        frame: f64,
        now: Instant,
    ) {
        let whole = frame.floor() as i64;
        let rate = project.rate();
        let sounding: Vec<Cue<'_>> = project
            .voices_at(frame)
            .into_iter()
            .filter_map(|voice| {
                let voiced = AudibleSource {
                    layer: voice.layer,
                    clip: voice.clip,
                    at: frame,
                    source: rate.time_at(voice.clip.source_position(frame)),
                    gain: voice.gain,
                };
                cue(project, live, voiced)
            })
            .collect();
        let coming: Vec<Cue<'_>> = upcoming(project, whole)
            .map(|at| {
                project
                    .voices_at(at as f64)
                    .into_iter()
                    .filter_map(|voice| {
                        let voiced = AudibleSource {
                            layer: voice.layer,
                            clip: voice.clip,
                            at: at as f64,
                            source: rate.start_of(voice.clip.source_at(at)),
                            gain: voice.gain,
                        };
                        cue(project, live, voiced)
                    })
                    .filter(|cue| {
                        !sounding
                            .iter()
                            .any(|source| source.strand.continues(&cue.strand))
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.voices.follow(&sounding, &coming, now);
    }
}

fn upcoming(project: &Project, frame: i64) -> Option<i64> {
    let reach = project.rate().frames_in(PREROLL).max(1);
    project
        .next_change(frame, Stream::Sound)
        .filter(|at| at - frame <= reach)
}

struct AudibleSource<'a> {
    layer: usize,
    clip: &'a Clip,
    at: f64,
    source: Duration,
    gain: f32,
}

fn cue<'a>(project: &'a Project, live: &LivePreview, voiced: AudibleSource<'_>) -> Option<Cue<'a>> {
    let AudibleSource {
        layer,
        clip,
        at,
        source,
        gain,
    } = voiced;
    let media = project.media(clip.media()?)?;
    Some(Cue {
        strand: SourceLine {
            layer,
            alignment: Alignment::for_clip(clip)?,
        },
        path: media.path(),
        source,
        level: clip.level_at(at, live.mix(clip.id()))?,
        gain,
    })
}
