use std::ops::Range;

use rikka_types::{Point, Rect};
use rikka_ui::{Axis, Track, Tracks};

use crate::lists::collection::spans::Spans;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Lines {
    Down,
    Across,
    Grid(usize),
    Tiles(f32),
}

impl Lines {
    pub(crate) fn along(self) -> Axis {
        match self {
            Self::Across => Axis::Horizontal,
            _ => Axis::Vertical,
        }
    }

    pub(crate) fn across(self, breadth: f32, gap: f32) -> usize {
        match self {
            Self::Down | Self::Across => 1,
            Self::Grid(lines) => lines.max(1),
            Self::Tiles(tile) => (((breadth + gap) / (tile + gap)).floor() as usize).max(1),
        }
    }

    pub(crate) fn breadth(self, room: f32, across: usize, gap: f32) -> f32 {
        match self {
            Self::Tiles(tile) => tile,
            _ => {
                let gaps = across.saturating_sub(1) as f32 * gap;
                ((room - gaps) / across.max(1) as f32).max(0.0)
            }
        }
    }
    pub(crate) fn tracks(self, band: Option<f32>) -> Tracks {
        let track = match self {
            Self::Tiles(tile) => Track::Fitting(tile),
            Self::Grid(lines) => Track::Even(Tracks::lines(lines)),
            _ => Track::Even(1),
        };
        Tracks::new(track).along(self.along()).band(band)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Surface<'a> {
    pub(crate) body: Rect,
    pub(crate) lines: Lines,
    pub(crate) offset: f32,
    pub(crate) across: usize,
    pub(crate) breadth: f32,
    pub(crate) gap: f32,
    pub(crate) count: usize,
    pub(crate) spans: &'a Spans,
}

impl Surface<'_> {
    pub fn visible(&self) -> Range<usize> {
        let along = self.lines.along();
        let bands = self.spans.count();
        if bands == 0 {
            return 0..0;
        }
        let first = self.spans.at(self.offset);
        let last = (self.spans.at(self.offset + along.room(self.body)) + 1).min(bands);
        (first * self.across).min(self.count)..(last * self.across).min(self.count)
    }

    pub fn cell(&self, index: usize) -> Rect {
        let along = self.lines.along();
        let band = index / self.across;
        let line = index % self.across;
        let start = self.spans.top(band);
        along.rect(
            self.body.origin,
            start - self.offset,
            (self.spans.top(band + 1) - start - self.gap).max(0.0),
            line as f32 * (self.breadth + self.gap),
            self.breadth,
        )
    }

    pub fn at(&self, point: Point) -> Option<usize> {
        if !self.body.contains(point) {
            return None;
        }
        let along = self.lines.along();
        let band = self
            .spans
            .at(along.coord(point) - along.coord(self.body.origin) + self.offset);
        let across = along.across();
        let reached = across.coord(point) - across.coord(self.body.origin);
        let line = (reached / (self.breadth + self.gap).max(f32::EPSILON)).floor() as usize;
        let index = band * self.across + line.min(self.across.saturating_sub(1));
        (index < self.count && self.cell(index).contains(point)).then_some(index)
    }
}
