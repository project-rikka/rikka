use std::time::Duration;

use rikka_compositor_caption::{Button, Part};
use rikka_compositor_desktop::WindowId;
use rikka_compositor_geometry::{Logical, Point};

const DOUBLE_PRESS_INTERVAL: Duration = Duration::from_millis(500);

const DOUBLE_PRESS_SLOP: f64 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct TitlePress {
    window: WindowId,
    at: Point<f64, Logical>,
    time: Duration,
}

#[derive(Debug, Default)]
pub(crate) struct CaptionPointer {
    hovered: Option<(WindowId, Part)>,
    pressed: Option<(WindowId, Button)>,
    last_title: Option<TitlePress>,
}

impl CaptionPointer {
    pub(crate) fn hover(&mut self, hovered: Option<(WindowId, Part)>) {
        self.hovered = hovered;
    }

    pub(crate) fn press(&mut self, pressed: Option<(WindowId, Button)>) {
        self.pressed = pressed;
    }

    pub(crate) fn lights(&self) -> [Option<(WindowId, Button)>; 2] {
        [self.lit(), self.pressed]
    }

    pub(crate) fn pressed(&self) -> Option<(WindowId, Button)> {
        self.pressed
    }

    pub(crate) fn lit(&self) -> Option<(WindowId, Button)> {
        match (self.pressed, self.hovered) {
            (Some(pressed), Some((window, Part::Button(button)))) => {
                (pressed == (window, button)).then_some(pressed)
            }
            (Some(_), _) => None,
            (None, Some((window, Part::Button(button)))) => Some((window, button)),
            (None, _) => None,
        }
    }

    pub(crate) fn is_held(&self, window: WindowId, button: Button) -> bool {
        self.pressed == Some((window, button)) && self.lit() == self.pressed
    }

    pub(crate) fn title_pressed(
        &mut self,
        window: WindowId,
        at: Point<f64, Logical>,
        time: Duration,
    ) -> bool {
        let doubled = self.last_title.is_some_and(|last| {
            last.window == window
                && time.saturating_sub(last.time) <= DOUBLE_PRESS_INTERVAL
                && (at.x - last.at.x).hypot(at.y - last.at.y) <= DOUBLE_PRESS_SLOP
        });
        self.last_title = (!doubled).then_some(TitlePress { window, at, time });
        doubled
    }

    pub(crate) fn forget(&mut self, window: WindowId) {
        if self.hovered.is_some_and(|(hovered, _)| hovered == window) {
            self.hovered = None;
        }
        if self.pressed.is_some_and(|(pressed, _)| pressed == window) {
            self.pressed = None;
        }
        if self.last_title.is_some_and(|last| last.window == window) {
            self.last_title = None;
        }
    }
}
