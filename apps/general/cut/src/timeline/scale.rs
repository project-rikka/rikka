use std::cell::Cell;
use std::rc::Rc;

use rikka_app::prelude::ScrollDelta;
use rikka_timeline::{ScrollExtent, TimeScale, ZoomRange};
use rikka_ui::Observers;

const RESTING: f64 = 2.0;
const ZOOMS: ZoomRange = ZoomRange {
    farthest: 0.0005,
    nearest: 48.0,
};
const STEP: f64 = 1.5;

#[derive(Clone, Debug, Default)]
pub(crate) struct Scale(Rc<Inner>);

#[derive(Debug)]
struct Inner {
    scale: Cell<TimeScale>,
    observers: Observers,
}

impl Default for Inner {
    fn default() -> Inner {
        Inner {
            scale: Cell::new(TimeScale::new(RESTING, ZOOMS)),
            observers: Observers::new(),
        }
    }
}

impl PartialEq for Scale {
    fn eq(&self, other: &Scale) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Scale {
    pub(crate) fn shown(&self) -> TimeScale {
        self.0.observers.read();
        self.0.scale.get()
    }

    fn update(&self, change: impl FnOnce(&mut TimeScale)) {
        let before = self.0.scale.get();
        let mut scale = before;
        change(&mut scale);
        if scale != before {
            self.0.scale.set(scale);
            self.0.observers.changed();
        }
    }

    pub(crate) fn per_frame(&self) -> f64 {
        self.shown().per_frame()
    }

    pub(crate) fn first(&self) -> f64 {
        self.shown().first()
    }

    pub(crate) fn width(&self) -> f64 {
        self.shown().width()
    }

    pub(crate) fn x_of(&self, frame: f64) -> f32 {
        self.shown().x_of(frame)
    }

    pub(crate) fn frame_at(&self, x: f32) -> f64 {
        self.shown().frame_at(x)
    }

    pub(crate) fn frames_across(&self, pixels: f32) -> f64 {
        self.shown().frames_across(pixels)
    }

    pub(crate) fn last(&self) -> f64 {
        self.shown().last()
    }

    pub(crate) fn level(&self) -> f32 {
        self.shown().level()
    }

    pub(crate) fn extent(&self) -> ScrollExtent {
        self.shown().extent()
    }

    pub(crate) fn meet(&self, width: f32, end: i64) {
        self.update(|scale| {
            scale.set_width(width);
            scale.set_end(end as f64);
        });
    }

    pub(crate) fn zoom(&self, closer: bool, around: f32) {
        let factor = if closer { STEP } else { 1.0 / STEP };
        self.update(|scale| scale.zoom_at(factor, around));
    }

    pub(crate) fn zoom_notches(&self, delta: ScrollDelta, around: f32) {
        self.update(|scale| scale.zoom_notches(delta.zoom(), around));
    }

    pub(crate) fn set_level(&self, level: f32, around: f32) {
        self.update(|scale| scale.set_level(level, around));
    }

    pub(crate) fn fit(&self, end: i64) {
        self.update(|scale| {
            scale.set_end(end as f64);
            scale.fit();
        });
    }

    pub(crate) fn show(&self, from: f64, to: f64) {
        self.update(|scale| scale.show(from, to));
    }

    pub(crate) fn scroll_by(&self, pixels: f32, end: i64) {
        self.update(|scale| {
            scale.set_end(end as f64);
            scale.scroll_by(pixels);
        });
    }

    pub(crate) fn scroll_to(&self, first: f64) {
        self.update(|scale| scale.scroll_to(first));
    }

    pub(crate) fn follow(&self, frame: f64) {
        self.update(|scale| scale.follow(frame));
    }
}
