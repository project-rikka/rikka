use rikka_keyframe::Frame;

use crate::scroll::ScrollExtent;

const OVERSCROLL: f64 = 0.5;
const FOLLOW_MARGIN: f64 = 0.1;
const FIT_MARGIN: f64 = 0.04;
const ZOOM_PER_NOTCH: f64 = 1.2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoomRange {
    pub farthest: f64,
    pub nearest: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeScale {
    per_frame: f64,
    first: f64,
    width: f64,
    end: f64,
    range: ZoomRange,
}

impl TimeScale {
    #[must_use]
    pub fn new(per_frame: f64, range: ZoomRange) -> TimeScale {
        TimeScale {
            per_frame: per_frame.clamp(range.farthest, range.nearest),
            first: 0.0,
            width: 0.0,
            end: 0.0,
            range,
        }
    }

    #[must_use]
    pub fn per_frame(&self) -> f64 {
        self.per_frame
    }

    #[must_use]
    pub fn first(&self) -> f64 {
        self.first
    }

    #[must_use]
    pub fn width(&self) -> f64 {
        self.width
    }

    #[must_use]
    pub fn end(&self) -> f64 {
        self.end
    }

    #[must_use]
    pub fn span(&self) -> f64 {
        self.width / self.per_frame
    }

    #[must_use]
    pub fn last(&self) -> f64 {
        self.first + self.span()
    }

    #[must_use]
    pub fn x_of(&self, frame: f64) -> f32 {
        ((frame - self.first) * self.per_frame) as f32
    }

    #[must_use]
    pub fn frame_at(&self, x: f32) -> f64 {
        self.first + f64::from(x) / self.per_frame
    }

    #[must_use]
    pub fn frames_across(&self, pixels: f32) -> f64 {
        f64::from(pixels) / self.per_frame
    }

    #[must_use]
    pub fn nearest<'k, K, F: Frame>(
        &self,
        keys: &'k [K],
        frame: impl Fn(&K) -> F,
        x: f32,
        reach: f32,
    ) -> Option<&'k K> {
        let at = self.frame_at(x);
        let within = self.frames_across(reach);
        let from = keys.partition_point(|key| frame(key).to_f64() < at - within);
        keys[from..]
            .iter()
            .take_while(|key| frame(key).to_f64() <= at + within)
            .min_by(|one, two| {
                (frame(one).to_f64() - at)
                    .abs()
                    .total_cmp(&(frame(two).to_f64() - at).abs())
            })
    }

    #[must_use]
    pub fn level(&self) -> f32 {
        let ZoomRange { farthest, nearest } = self.range;
        ((self.per_frame / farthest).ln() / (nearest / farthest).ln()) as f32
    }

    #[must_use]
    pub fn extent(&self) -> ScrollExtent {
        let span = self.span();
        ScrollExtent {
            at: self.first,
            shown: span,
            content: self.furthest().max(self.first) + span,
        }
    }

    pub fn set_width(&mut self, width: f32) {
        self.width = f64::from(width.max(0.0));
        self.settle();
    }

    pub fn set_end(&mut self, end: f64) {
        self.end = end.max(0.0);
        self.settle();
    }

    pub fn zoom_at(&mut self, factor: f64, around: f32) {
        let pinned = self.frame_at(around);
        self.per_frame = (self.per_frame * factor).clamp(self.range.farthest, self.range.nearest);
        self.first = pinned - f64::from(around) / self.per_frame;
        self.settle();
    }

    pub fn zoom_notches(&mut self, notches: f32, around: f32) {
        self.zoom_at(ZOOM_PER_NOTCH.powf(f64::from(notches)), around);
    }

    pub fn set_level(&mut self, level: f32, around: f32) {
        let ZoomRange { farthest, nearest } = self.range;
        let per_frame = farthest * (nearest / farthest).powf(f64::from(level.clamp(0.0, 1.0)));
        self.zoom_at(per_frame / self.per_frame, around);
    }

    pub fn fit(&mut self) {
        if self.end <= 0.0 || self.width <= 0.0 {
            return;
        }
        self.per_frame = (self.width / (self.end * (1.0 + FIT_MARGIN)))
            .clamp(self.range.farthest, self.range.nearest);
        self.first = 0.0;
    }

    pub fn show(&mut self, from: f64, to: f64) {
        if to <= from || self.width <= 0.0 {
            return;
        }
        self.per_frame = (self.width / (to - from)).clamp(self.range.farthest, self.range.nearest);
        self.first = from;
        self.settle();
    }

    pub fn scroll_by(&mut self, pixels: f32) {
        self.first += self.frames_across(pixels);
        self.settle();
    }

    pub fn scroll_to(&mut self, first: f64) {
        self.first = first;
        self.settle();
    }

    pub fn follow(&mut self, frame: f64) {
        let span = self.span();
        if span <= 0.0
            || (frame >= self.first && frame <= self.first + span * (1.0 - FOLLOW_MARGIN))
        {
            return;
        }
        self.first = frame - span * FOLLOW_MARGIN;
        self.settle();
    }

    fn furthest(&self) -> f64 {
        let span = self.span();
        let beyond = (self.end - span).max(0.0);
        beyond + beyond.min(span * OVERSCROLL)
    }

    fn settle(&mut self) {
        self.first = self.first.min(self.furthest()).max(0.0);
    }
}
