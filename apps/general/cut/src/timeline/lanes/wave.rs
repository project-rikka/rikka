use cut_project::{Clip, Rate};
use cut_waveform::{Peaks, Scale as Loudness, Summary};
use rikka_app::prelude::*;

use crate::timeline::scale::Scale;

const COLUMN: f32 = 1.0;
const SPLIT_FROM: f32 = 44.0;
const PAD: f32 = 1.5;
const PEAK: f32 = 0.5;
const BODY: f32 = 0.85;
const MIDDLE: f32 = 0.18;
const MUTED: f32 = 0.35;

pub(super) struct Wave<'a> {
    pub(super) loudness: Loudness,
    pub(super) rate: Rate,
    pub(super) scale: &'a Scale,
    pub(super) bounds: Rect,
    pub(super) visible: Rect,
}

#[derive(Clone, Copy)]
enum Lane {
    Channel(u16),
    Mixed,
}

struct Pen {
    loudness: Loudness,
    gain: f32,
    middle: f32,
    half: f32,
    ink: Color,
    danger: Color,
}

pub(super) struct AudioLevel<'a> {
    pub(super) gain: &'a dyn Fn(f64) -> f32,
    pub(super) muted: bool,
}

impl Wave<'_> {
    pub(super) fn paint(
        &self,
        ctx: &mut PaintCtx<'_>,
        peaks: &Peaks,
        clip: &Clip,
        audio: &AudioLevel<'_>,
        band: Rect,
        ink: Color,
    ) {
        let Some(visible) = band.intersection(&self.visible) else {
            return;
        };
        let lanes: Vec<Lane> = match peaks.channels() {
            2 if band.height() >= SPLIT_FROM => vec![Lane::Channel(0), Lane::Channel(1)],
            1 => vec![Lane::Channel(0)],
            _ => vec![Lane::Mixed],
        };
        let samples = f64::from(peaks.rate()) / self.rate.per_second();
        let sample_at = |x: f32| {
            let frame = self.scale.frame_at(x - self.bounds.x());
            (clip.source_position(frame) * samples).max(0.0) as u64
        };
        let lane_height = band.height() / lanes.len() as f32;
        let opacity = match audio.muted {
            true => MUTED,
            false => 1.0,
        };
        let danger = palette().danger;
        let columns: Vec<(f32, f32)> = (0..)
            .map(|step| visible.x().floor() + step as f32 * COLUMN)
            .take_while(|x| *x < visible.max_x())
            .map(|x| (x, (audio.gain)(self.scale.frame_at(x - self.bounds.x()))))
            .collect();
        ctx.with_opacity(opacity, |ctx| {
            for (index, lane) in lanes.iter().enumerate() {
                let top = band.y() + lane_height * index as f32;
                let mut pen = Pen {
                    loudness: self.loudness,
                    gain: 1.0,
                    middle: top + lane_height / 2.0,
                    half: (lane_height / 2.0 - PAD).max(0.5),
                    ink,
                    danger,
                };
                ctx.draw_quad(
                    Quad::new(Rect::new(
                        visible.x(),
                        pen.middle - 0.5,
                        visible.width(),
                        1.0,
                    ))
                    .fill(ink.scale_alpha(MIDDLE)),
                );
                for (x, gain) in &columns {
                    let from = sample_at(*x);
                    let frames = from..sample_at(x + COLUMN).max(from + 1);
                    let summary = match lane {
                        Lane::Channel(channel) => peaks.summary(*channel, frames),
                        Lane::Mixed => peaks.mixed(frames),
                    };
                    if let Some(summary) = summary {
                        pen.gain = *gain;
                        pen.column(ctx, &summary, *x);
                    }
                }
            }
        });
    }
}

impl Pen {
    fn column(&self, ctx: &mut PaintCtx<'_>, summary: &Summary, x: f32) {
        let reach = |amplitude: f32| self.loudness.level(amplitude.max(0.0), self.gain) * self.half;
        let up = reach(summary.high);
        let down = reach(-summary.low);
        let loud = reach(summary.rms);
        let peak = match Loudness::clips(summary.high.max(-summary.low), self.gain) {
            true => self.danger,
            false => self.ink.scale_alpha(PEAK),
        };
        ctx.draw_quad(
            Quad::new(Rect::new(x, self.middle - up, COLUMN, (up + down).max(0.5))).fill(peak),
        );
        if loud > 0.0 {
            ctx.draw_quad(
                Quad::new(Rect::new(x, self.middle - loud, COLUMN, loud * 2.0))
                    .fill(self.ink.scale_alpha(BODY)),
            );
        }
    }
}
