use std::ops::Range;

use crate::lists::collection::Spans;

const LOOKAHEAD: f32 = 1.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Heading {
    #[default]
    Unknown,
    Forward,
    Back,
}

impl Heading {
    pub(crate) fn from_travel(travel: f32) -> Heading {
        match travel {
            travel if travel > 0.0 => Heading::Forward,
            travel if travel < 0.0 => Heading::Back,
            _ => Heading::Unknown,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Ahead {
    zone: Range<usize>,
    heading: Heading,
}

impl Ahead {
    pub(crate) fn new(spans: &Spans, visible: &Range<usize>, heading: Heading, room: f32) -> Ahead {
        let reach = room.max(0.0) * LOOKAHEAD;
        let count = spans.count();
        let zone = match heading {
            Heading::Unknown => visible.end..visible.end,
            Heading::Forward if visible.end >= count => visible.end..visible.end,
            Heading::Forward => {
                let end = spans.at(spans.top(visible.end) + reach) + 1;
                visible.end..end.clamp(visible.end, count)
            }
            Heading::Back => {
                let start = spans.at((spans.top(visible.start) - reach).max(0.0));
                start.min(visible.start)..visible.start
            }
        };
        Ahead { zone, heading }
    }

    pub(crate) fn around(&self, visible: &Range<usize>) -> Range<usize> {
        visible.start.min(self.zone.start)..visible.end.max(self.zone.end)
    }

    pub(crate) fn lacks(&self, window: &Range<usize>) -> bool {
        !self.zone.is_empty() && (window.start > self.zone.start || window.end < self.zone.end)
    }

    pub(crate) fn extend(
        &self,
        window: Range<usize>,
        built: &Range<usize>,
        prefetching: bool,
    ) -> Range<usize> {
        if self.zone.is_empty() {
            return window;
        }
        match self.heading {
            Heading::Unknown => window,
            Heading::Forward => {
                let mut end = window.end;
                if built.start <= end {
                    end = end.max(built.end.min(self.zone.end));
                }
                if prefetching && end < self.zone.end {
                    end += 1;
                }
                window.start..end
            }
            Heading::Back => {
                let mut start = window.start;
                if built.end >= start {
                    start = start.min(built.start.max(self.zone.start));
                }
                if prefetching && start > self.zone.start {
                    start -= 1;
                }
                start..window.end
            }
        }
    }
}
