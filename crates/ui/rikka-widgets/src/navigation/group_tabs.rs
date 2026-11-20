use std::ops::Range;

use rikka_types::{Point, Rect};
use rikka_ui::PaintCtx;
use rikka_ui::text::Label;

use crate::control::ControlSize;
use crate::controls::track::{self, Facet, Track, TrackState};

const NARROWEST: f32 = 92.0;

pub(crate) struct Group {
    pub(crate) label: Label,
    pub(crate) span: Range<usize>,
}

pub(crate) struct GroupTabs<'a> {
    pub(crate) groups: &'a [Group],
    pub(crate) shown: usize,
    pub(crate) size: ControlSize,
}

impl GroupTabs<'_> {
    pub(crate) fn height(size: ControlSize) -> f32 {
        Track::height(size)
    }

    pub(crate) fn width(count: usize) -> f32 {
        count as f32 * NARROWEST + track::PADDING * 2.0
    }

    pub(crate) fn at(&self, panel: Rect, position: Point) -> Option<usize> {
        let band = self.band(panel);
        band.contains(position)
            .then(|| self.track().spans(band).at(position.x))
    }

    pub(crate) fn paint(&self, ctx: &mut PaintCtx<'_>, state: &mut TrackState, band: Rect) {
        self.track().paint(
            ctx,
            state,
            band,
            |index| self.facet(index),
            self.shown,
            false,
        );
    }

    fn band(&self, panel: Rect) -> Rect {
        Rect::new(
            panel.x(),
            panel.y(),
            panel.width(),
            GroupTabs::height(self.size),
        )
    }

    fn facet(&self, index: usize) -> Facet {
        Facet::Lettered(self.groups[index].label.clone())
    }

    fn track(&self) -> Track {
        Track::new(
            self.size,
            &rikka_ui::theme(),
            (0..self.groups.len()).map(|index| self.facet(index)),
        )
    }
}
