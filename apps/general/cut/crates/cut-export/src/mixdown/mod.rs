mod strand;

use std::collections::BTreeSet;
use std::sync::Arc;

use cut_project::Alignment;
use cut_project::{AudibleClip, Clip, MediaId, Project};
use rikka_audio::Pan;

use strand::Strand;

pub const CHANNELS: u16 = 2;
const BEFORE_NEXT: f64 = 1e-6;

pub struct Mixdown {
    project: Arc<Project>,
    first: i64,
    sample_rate: u32,
    produced: u64,
    strands: Vec<Strand>,
    undecodable: BTreeSet<MediaId>,
}

impl std::fmt::Debug for Mixdown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mixdown")
            .field("first", &self.first)
            .field("sample_rate", &self.sample_rate)
            .field("produced", &self.produced)
            .field("unheard", &self.undecodable)
            .finish_non_exhaustive()
    }
}

struct Sounding {
    layer: usize,
    alignment: Alignment,
    media: MediaId,
    from: f32,
    to: f32,
    pan: f32,
    source: f64,
}

impl Mixdown {
    #[must_use]
    pub fn new(project: Arc<Project>, first: i64, sample_rate: u32) -> Mixdown {
        Mixdown {
            project,
            first: first.max(0),
            sample_rate: sample_rate.max(1),
            produced: 0,
            strands: Vec::new(),
            undecodable: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    #[must_use]
    pub fn undecodable(&self) -> &BTreeSet<MediaId> {
        &self.undecodable
    }

    pub fn fill(&mut self, out: &mut [f32]) {
        let channels = usize::from(CHANNELS);
        out.fill(0.0);
        let mut done = 0;
        let frames = out.len() / channels;
        while done < frames {
            let (frame, boundary) = self.frame_at(self.produced);
            let count = ((boundary - self.produced) as usize).min(frames - done);
            let block = &mut out[done * channels..(done + count) * channels];
            self.mix(frame, count, block);
            self.produced += count as u64;
            done += count;
        }
        for sample in out.iter_mut() {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }

    fn frame_at(&self, sample: u64) -> (i64, u64) {
        let rate = self.project.rate();
        let numerator = u128::from(rate.numerator());
        let per = u128::from(rate.denominator()) * u128::from(self.sample_rate);
        let elapsed = u128::from(sample) * numerator / per;
        let boundary = ((elapsed + 1) * per).div_ceil(numerator);
        (self.first + elapsed as i64, boundary as u64)
    }

    fn position(&self, sample: u64) -> f64 {
        let rate = self.project.rate();
        self.first as f64 + sample as f64 * rate.per_second() / f64::from(self.sample_rate)
    }

    fn mix(&mut self, frame: i64, count: usize, block: &mut [f32]) {
        let start = self.position(self.produced).max(frame as f64);
        let end = self
            .position(self.produced + count as u64)
            .min(frame as f64 + 1.0 - BEFORE_NEXT);
        let sounding = self.sounding(start, end);
        for strand in &mut self.strands {
            strand.used = false;
        }
        for voice in sounding {
            let at = match self.strands.iter().position(|strand| {
                strand.layer == voice.layer && strand.alignment.continues(&voice.alignment)
            }) {
                Some(at) => at,
                None => {
                    let opened = self.open(&voice);
                    self.strands.push(opened);
                    self.strands.len() - 1
                }
            };
            let strand = &mut self.strands[at];
            strand.used = true;
            strand.alignment = voice.alignment;
            let pan = Pan::new(voice.pan, strand.channels(), CHANNELS);
            if let Some(pulled) = strand.pull(count) {
                pan.mix(pulled, voice.from..voice.to, block);
            }
            if strand.failed() {
                self.undecodable.insert(voice.media);
            }
        }
        self.strands.retain(|strand| strand.used);
    }

    fn sounding(&self, start: f64, end: f64) -> Vec<Sounding> {
        let project = &self.project;
        let ending = project.voices_at(end);
        project
            .voices_at(start)
            .into_iter()
            .filter_map(|voice| {
                let to = ending
                    .iter()
                    .find(|other| other.layer == voice.layer && other.clip.id() == voice.clip.id())
                    .map_or(voice.gain, |other| other.gain);
                sounding(project, &voice, to, (start, end))
            })
            .collect()
    }

    fn open(&mut self, voice: &Sounding) -> Strand {
        let rate = self.project.rate();
        let Some(media) = self.project.media(voice.media) else {
            return Strand::silent(voice.layer, voice.alignment);
        };
        match Strand::open(
            voice.layer,
            voice.alignment,
            media.path(),
            rate.time_at(voice.source),
            self.sample_rate,
        ) {
            Ok(strand) => strand,
            Err(error) => {
                tracing::warn!(%error, path = %media.path().display(), "cannot decode sound");
                self.undecodable.insert(voice.media);
                Strand::silent(voice.layer, voice.alignment)
            }
        }
    }
}

fn sounding(
    project: &Project,
    voice: &AudibleClip<'_>,
    to: f32,
    (start, end): (f64, f64),
) -> Option<Sounding> {
    let clip: &Clip = voice.clip;
    let media = clip.media()?;
    project.media(media)?;
    let (from, until) = (clip.level_at(start, None)?, clip.level_at(end, None)?);
    Some(Sounding {
        layer: voice.layer,
        alignment: Alignment::for_clip(clip)?,
        media,
        from: voice.gain * from.gain,
        to: to * until.gain,
        pan: from.pan,
        source: clip.source_position(start),
    })
}
