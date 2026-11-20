use rikka_input::evdev;
use rikka_input::{ScrollPhase, ScrollSource};
use rikka_types::{Point, SCROLL_LINE, ScrollDelta};
use rikka_wayland_client::wl_pointer::{Axis, AxisRelativeDirection};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Scroll {
    pub(crate) absolute: f64,
    pub(crate) discrete: i32,
    pub(crate) value120: i32,
    pub(crate) direction: Option<AxisRelativeDirection>,
    pub(crate) stop: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct AxisFrame {
    pub(crate) horizontal: Scroll,
    pub(crate) vertical: Scroll,
}

impl Scroll {
    fn merge(&self, other: &Scroll) -> Option<Scroll> {
        let direction = match (self.direction, other.direction) {
            (None, either) | (either, None) => either,
            (Some(one), Some(other)) if one == other => Some(one),
            _ => return None,
        };
        Some(Scroll {
            absolute: self.absolute + other.absolute,
            discrete: self.discrete + other.discrete,
            value120: self.value120 + other.value120,
            direction,
            stop: self.stop || other.stop,
        })
    }

    fn is_discrete(&self) -> bool {
        self.value120 != 0 || self.discrete != 0
    }

    fn amount(&self) -> f32 {
        if self.value120 != 0 {
            return evdev::value120_to_pixels(self.value120);
        }
        if self.discrete != 0 {
            return self.discrete as f32 * SCROLL_LINE;
        }
        self.absolute as f32
    }
}

impl AxisFrame {
    pub(crate) fn on_axis(
        axis: Result<Axis, rikka_wayland_client::UnknownValue>,
        set: impl FnOnce(&mut Scroll),
    ) -> Option<AxisFrame> {
        let mut scrolled = AxisFrame::default();
        match axis {
            Ok(Axis::VerticalScroll) => set(&mut scrolled.vertical),
            Ok(Axis::HorizontalScroll) => set(&mut scrolled.horizontal),
            unknown => {
                tracing::warn!("unknown pointer axis: {unknown:?}");
                return None;
            }
        }
        Some(scrolled)
    }

    pub(crate) fn merge(&self, other: &AxisFrame) -> Option<AxisFrame> {
        Some(AxisFrame {
            horizontal: self.horizontal.merge(&other.horizontal)?,
            vertical: self.vertical.merge(&other.vertical)?,
        })
    }

    pub(crate) fn delta(&self) -> ScrollDelta {
        let natural = [self.horizontal.direction, self.vertical.direction]
            .contains(&Some(AxisRelativeDirection::Inverted));
        ScrollDelta::from_offset(Point::new(self.horizontal.amount(), self.vertical.amount()))
            .with_natural(natural)
    }

    pub(crate) fn source(&self, scrolling: &mut bool) -> ScrollSource {
        if self.horizontal.is_discrete() || self.vertical.is_discrete() {
            return ScrollSource::Wheel;
        }
        let phase = match (self.horizontal.stop || self.vertical.stop, *scrolling) {
            (true, _) => ScrollPhase::Ended,
            (false, true) => ScrollPhase::Moved,
            (false, false) => ScrollPhase::Began,
        };
        *scrolling = phase != ScrollPhase::Ended;
        ScrollSource::Continuous(phase)
    }
}
