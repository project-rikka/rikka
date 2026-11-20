use super::{Blending, Breaks, Clip};
use crate::file::Record;
use crate::read::modifier::Notes;

pub(super) const REPLACE: i64 = 0;
pub(super) const ADD: i64 = 1;
pub(super) const SUBTRACT: i64 = 2;
pub(super) const MULTIPLY: i64 = 3;
pub(super) const COMBINE: i64 = 4;

const HOLD: i64 = 0;
const NOTHING: i64 = 2;
const CLIP: i64 = 0;
const META: i64 = 2;
const USER_INFLUENCE: i64 = 1 << 5;
const REVERSE: i64 = 1 << 11;
const STRIP_MUTED: i64 = 1 << 12;
const TRACK_MUTED: i64 = 1 << 2;
const TRACK_SOLO: i64 = 1 << 3;
const TRACK_DISABLED: i64 = 1 << 10;
const SOLO_TRACK: i64 = 1;
const EVALUATION_OFF: i64 = 1 << 1;

pub(super) struct Strip {
    clip: Clip,
    start: f32,
    end: f32,
    action_start: f32,
    action_end: f32,
    scale: f32,
    blend_in: f32,
    blend_out: f32,
    extend: i64,
    reverse: bool,
    influence: Option<f32>,
    mode: i64,
}

impl Strip {
    fn time(&self, frame: f32) -> f32 {
        let length = match self.action_end - self.action_start {
            0.0 => 1.0,
            length => length,
        };
        let span = length * self.scale;
        let elapsed = frame - self.start;
        let within = elapsed % span;
        match self.reverse {
            true => match within == 0.0 && frame > self.start {
                true => self.action_start,
                false => self.action_end - within / self.scale,
            },
            false => match within == 0.0 && frame > self.start {
                true => self.action_end,
                false => self.action_start + within / self.scale,
            },
        }
    }

    fn breaks(&self, wanted: &dyn Fn(&str) -> bool, breaks: &mut Breaks) -> bool {
        let edges = [
            self.start,
            self.end,
            self.start + self.blend_in,
            self.end - self.blend_out,
        ];
        if !edges.into_iter().all(|frame| breaks.push(frame)) {
            return false;
        }
        let length = match self.action_end - self.action_start {
            0.0 => 1.0,
            length => length,
        };
        let span = length * self.scale;
        if !span.is_finite() || span <= 0.0 {
            return true;
        }
        let repeats = ((self.end - self.start) / span)
            .ceil()
            .clamp(1.0, breaks.capacity() as f32) as usize;
        let window = (
            self.action_start.min(self.action_end),
            self.action_start.max(self.action_end),
        );
        for curve in self.clip.curves.iter().filter(|curve| wanted(&curve.path)) {
            let mut room = true;
            curve.breaks(window.0, window.1, &mut |time| {
                let within = match self.reverse {
                    true => self.action_end - time,
                    false => time - self.action_start,
                } * self.scale;
                for repeat in 0..repeats {
                    let frame = self.start + repeat as f32 * span + within;
                    if frame > self.end {
                        break;
                    }
                    if !breaks.push(frame) {
                        room = false;
                        return false;
                    }
                }
                true
            });
            if !room {
                return false;
            }
        }
        true
    }

    fn weight(&self, frame: f32) -> f32 {
        if let Some(influence) = self.influence {
            return influence;
        }
        if self.blend_in > 0.0 && frame <= self.start + self.blend_in {
            return ((frame - self.start) / self.blend_in).clamp(0.0, 1.0);
        }
        if self.blend_out > 0.0 && frame >= self.end - self.blend_out {
            return ((self.end - frame) / self.blend_out).clamp(0.0, 1.0);
        }
        1.0
    }
}

pub(super) struct Track {
    strips: Vec<Strip>,
}

impl Track {
    pub(super) fn at(&self, frame: f32) -> Option<(&Clip, f32, Blending)> {
        let strip_frame = self
            .strips
            .iter()
            .find(|strip| strip.start <= frame && frame <= strip.end)
            .map(|strip| (strip, frame))
            .or_else(|| {
                let first = self.strips.first()?;
                (frame < first.start && first.extend == HOLD).then_some((first, first.start))
            })
            .or_else(|| {
                self.strips
                    .iter()
                    .rev()
                    .find(|strip| strip.end < frame)
                    .filter(|strip| strip.extend != NOTHING)
                    .map(|strip| (strip, strip.end))
            });
        let (strip, at) = strip_frame?;
        Some((
            &strip.clip,
            strip.time(at),
            Blending {
                mode: strip.mode,
                influence: strip.weight(at),
            },
        ))
    }

    pub(super) fn breaks(&self, wanted: &dyn Fn(&str) -> bool, breaks: &mut Breaks) -> bool {
        self.strips.iter().all(|strip| strip.breaks(wanted, breaks))
    }

    pub(super) fn clips(&self) -> impl Iterator<Item = &Clip> {
        self.strips.iter().map(|strip| &strip.clip)
    }
}

pub(super) fn tracks(animation: &Record<'_>, name: &str, notes: &mut Notes) -> Vec<Track> {
    let flag = animation.int("flag").unwrap_or(0);
    if flag & EVALUATION_OFF != 0 {
        return Vec::new();
    }
    let records = animation.list("nla_tracks");
    let solo = flag & SOLO_TRACK != 0;
    records
        .iter()
        .filter(|track| {
            let track_flag = track.int("flag").unwrap_or(0);
            track_flag & (TRACK_MUTED | TRACK_DISABLED) == 0
                && (!solo || track_flag & TRACK_SOLO != 0)
        })
        .map(|track| {
            let mut strips = Vec::new();
            for strip in track.list("strips") {
                gather(&strip, name, notes, &mut strips, 0);
            }
            Track { strips }
        })
        .filter(|track| !track.strips.is_empty())
        .collect()
}

fn gather(strip: &Record<'_>, name: &str, notes: &mut Notes, out: &mut Vec<Strip>, depth: usize) {
    let flag = strip.int("flag").unwrap_or(0);
    if flag & STRIP_MUTED != 0 || depth > 8 {
        return;
    }
    match strip.int("type").unwrap_or(CLIP) {
        CLIP => {
            let Some(action) = strip.id("act") else {
                return;
            };
            let clip = Clip::of_action(
                &action,
                strip.int("action_slot_handle").unwrap_or(0),
                name,
                notes,
            );
            let scale = strip
                .float("scale")
                .map(f32::abs)
                .filter(|scale| *scale > 0.0)
                .unwrap_or(1.0);
            out.push(Strip {
                clip,
                start: strip.float("start").unwrap_or(0.0),
                end: strip.float("end").unwrap_or(0.0),
                action_start: strip.float("actstart").unwrap_or(0.0),
                action_end: strip.float("actend").unwrap_or(0.0),
                scale,
                blend_in: strip.float("blendin").unwrap_or(0.0),
                blend_out: strip.float("blendout").unwrap_or(0.0),
                extend: strip.int("extendmode").unwrap_or(HOLD),
                reverse: flag & REVERSE != 0,
                influence: (flag & USER_INFLUENCE != 0)
                    .then(|| strip.float("influence").unwrap_or(1.0)),
                mode: strip.int("blendmode").unwrap_or(REPLACE),
            });
        }
        META => {
            for inner in strip.list("strips") {
                gather(&inner, name, notes, out, depth + 1);
            }
        }
        _ => notes.unsupported(format!("NLA transitions of \"{name}\" not evaluated")),
    }
}
