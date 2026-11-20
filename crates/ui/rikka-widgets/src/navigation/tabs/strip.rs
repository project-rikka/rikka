use std::ops::Range;

use rikka_types::{Point, Rect, Size};
use rikka_ui::scroll::bar;

use crate::control::ControlSize;

pub(super) const PADDING: f32 = 4.0;

const MAX_WIDTH: f32 = 240.0;

const MIN_RATIO: f32 = 3.0;

const CLOSE_TARGET: f32 = 3.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct TabEntrance {
    pub(super) index: usize,
    pub(super) grown: f32,
}

impl TabEntrance {
    fn missing(self) -> f32 {
        1.0 - self.grown.clamp(0.0, 1.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Strip {
    step: ControlSize,
    band: Rect,
    lane: Rect,
    tab: f32,
    count: usize,
    offset: f32,
    opening: Option<TabEntrance>,
    new: Option<Rect>,
    back: Option<Rect>,
    forward: Option<Rect>,
}

impl Strip {
    pub(super) fn new(
        bounds: Rect,
        step: ControlSize,
        count: usize,
        opens: bool,
        caption: bool,
        offset: f32,
        opening: Option<TabEntrance>,
    ) -> Self {
        let inset = if caption { 0.0 } else { PADDING };
        let band = Rect::new(
            bounds.x() + inset,
            bounds.y() + PADDING,
            (bounds.width() - inset * 2.0).max(0.0),
            (bounds.height() - PADDING * 2.0).max(0.0),
        );
        let button = step.height();
        let room = (band.width() - if opens { button } else { 0.0 }).max(0.0);
        let scrolls = count as f32 * min_width(step) > room;

        let lane = Rect::new(
            band.x() + if scrolls { button } else { 0.0 },
            band.y(),
            (room - if scrolls { button * 2.0 } else { 0.0 }).max(0.0),
            band.height(),
        );
        let tab = if scrolls {
            min_width(step)
        } else {
            (room / count.max(1) as f32).min(MAX_WIDTH)
        };
        let seat = |x: f32| Rect::new(x, band.y(), button, band.height());
        let new = opens.then(|| {
            if scrolls {
                seat(band.max_x() - button)
            } else {
                seat(lane.x() + tab * count as f32)
            }
        });

        let opening = opening.filter(|opening| opening.index < count && opening.grown < 1.0);
        Self {
            step,
            band,
            lane,
            tab,
            count,
            offset: offset.clamp(
                0.0,
                max_offset(tab, occupied_width(count, opening), lane.width()),
            ),
            opening,
            new,
            back: scrolls.then(|| seat(band.x())),
            forward: scrolls.then(|| seat(lane.max_x())),
        }
    }

    pub(super) fn band(&self) -> Rect {
        self.band
    }

    pub(super) fn lane(&self) -> Rect {
        self.lane
    }

    pub(super) fn width(&self) -> f32 {
        self.tab
    }

    pub(super) fn offset(&self) -> f32 {
        self.offset
    }

    pub(super) fn new_rect(&self) -> Option<Rect> {
        self.new
    }

    pub(super) fn back_rect(&self) -> Option<Rect> {
        self.back
    }

    pub(super) fn forward_rect(&self) -> Option<Rect> {
        self.forward
    }

    pub(super) fn scrolls(&self) -> bool {
        self.back.is_some()
    }

    pub(super) fn tab_rect(&self, at: f32) -> Rect {
        let (shift, narrower) = match self.opening {
            Some(opening) => {
                let past = (at - opening.index as f32).clamp(0.0, 1.0);
                let near = 1.0 - (at - opening.index as f32).abs().min(1.0);
                (opening.missing() * past, opening.missing() * near)
            }
            None => (0.0, 0.0),
        };
        Rect::new(
            self.lane.x() - self.offset + self.tab * (at - shift),
            self.band.y(),
            self.tab * (1.0 - narrower),
            self.band.height(),
        )
    }

    pub(super) fn close_rect(&self, at: f32) -> Rect {
        let rect = self.tab_rect(at);
        let side = self.step.icon_size();
        let center = Point::new(rect.max_x() - self.step.gap() - side / 2.0, rect.center().y);
        Rect::from_center(center, Size::new(side, side)).inflate(CLOSE_TARGET)
    }

    pub(super) fn index_at(&self, point: Point) -> Option<usize> {
        if self.tab <= 0.0 || !self.lane.contains(point) {
            return None;
        }
        self.visible()
            .find(|index| self.tab_rect(*index as f32).contains(point))
    }

    pub(super) fn max_offset(&self) -> f32 {
        max_offset(
            self.tab,
            occupied_width(self.count, self.opening),
            self.lane.width(),
        )
    }

    pub(super) fn visible(&self) -> Range<usize> {
        if self.tab <= 0.0 {
            return 0..0;
        }
        let first = (self.offset / self.tab) as usize;
        let last = ((self.offset + self.lane.width()) / self.tab).ceil() as usize + 1;
        first.saturating_sub(1)..last.min(self.count)
    }

    pub(super) fn reveal(&self, index: usize) -> f32 {
        let rect = self.tab_rect(index as f32);
        let start = rect.x() - self.lane.x() + self.offset;
        bar::reveal(self.offset, start, rect.width(), self.lane.width())
    }
}

fn occupied_width(count: usize, opening: Option<TabEntrance>) -> f32 {
    count as f32 - opening.map_or(0.0, TabEntrance::missing)
}

fn min_width(step: ControlSize) -> f32 {
    (step.icon_size() + step.gap() * 2.0) * MIN_RATIO
}

fn max_offset(tab: f32, taken: f32, lane: f32) -> f32 {
    (tab * taken - lane).max(0.0)
}
