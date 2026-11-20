use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use rikka_session::gestures::{Axis, Swipe};
use rikka_ui::{NodeHandle, Watchers};

pub(super) const OPENING: f32 = 300.0;

pub(super) const PASSING: f32 = 400.0;

const SAMPLED_MS: u32 = 100;

const FLUNG: f32 = 0.4;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) enum SwipeState {
    #[default]
    Idle,
    Tracking(Track),
    Released(Track),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Track {
    pub(crate) id: u64,
    pub(crate) axis: Axis,
    pub(crate) output: Option<Rc<str>>,
    pub(crate) is_open: bool,
    pub(crate) travel: f32,
    pub(crate) speed: f32,
}

impl Track {
    #[must_use]
    pub(crate) fn spread(&self) -> f32 {
        let start = if self.is_open { 1.0 } else { 0.0 };
        (start + self.travel / OPENING).clamp(0.0, 1.0)
    }

    #[must_use]
    pub(super) fn stays_open(&self) -> bool {
        let speed = self.speed / OPENING;
        match speed.abs() >= FLUNG {
            true => speed > 0.0,
            false => self.spread() >= 0.5,
        }
    }

    #[must_use]
    pub(super) fn passed(&self) -> f32 {
        self.travel / PASSING
    }

    #[must_use]
    pub(crate) fn step(&self) -> i32 {
        step(self.passed(), self.speed / PASSING)
    }

    #[must_use]
    pub(super) fn sideways_on(&self, output: &str) -> bool {
        self.axis == Axis::Horizontal && self.output.as_deref() == Some(output)
    }
}

#[must_use]
pub(crate) fn step(passed: f32, speed: f32) -> i32 {
    let leaning = match speed.abs() >= FLUNG {
        true => speed,
        false if passed.abs() >= 0.5 => passed,
        false => 0.0,
    };
    match leaning {
        lean if lean > 0.0 => 1,
        lean if lean < 0.0 => -1,
        _ => 0,
    }
}

#[derive(Debug, Default)]
pub(super) struct Sampler(VecDeque<(u32, f32)>);

impl Sampler {
    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }

    pub(crate) fn push(&mut self, millis: u32, along: f32) {
        self.0.push_back((millis, along));
        while self
            .0
            .front()
            .is_some_and(|(at, _)| millis.wrapping_sub(*at) > SAMPLED_MS)
        {
            self.0.pop_front();
        }
    }

    #[must_use]
    pub(crate) fn speed(&self) -> f32 {
        let (Some((first, _)), Some((last, _))) = (self.0.front(), self.0.back()) else {
            return 0.0;
        };
        let span = last.wrapping_sub(*first);
        if span == 0 {
            return 0.0;
        }
        let travelled: f32 = self.0.iter().skip(1).map(|(_, along)| along).sum();
        travelled * 1000.0 / span as f32
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct Swipes(Rc<Inner>);

#[derive(Debug, Default)]
struct Inner {
    swiped: RefCell<SwipeState>,
    samples: RefCell<Sampler>,
    minted: Cell<u64>,
    watchers: Watchers,
}
impl Swipes {
    #[must_use]
    pub(crate) fn now(&self) -> SwipeState {
        self.0.swiped.borrow().clone()
    }

    pub(crate) fn watch(&self, watcher: NodeHandle) {
        self.0.watchers.watch(watcher);
    }

    pub(super) fn handle_swipe(&self, swipe: Swipe, is_open: bool) -> SwipeState {
        match swipe {
            Swipe::Began { axis, output } => {
                let id = self.0.minted.get() + 1;
                self.0.minted.set(id);
                self.0.samples.borrow_mut().clear();
                *self.0.swiped.borrow_mut() = SwipeState::Tracking(Track {
                    id,
                    axis,
                    output: output.map(Rc::from),
                    is_open,
                    travel: 0.0,
                    speed: 0.0,
                });
            }
            Swipe::Moved { dx, dy, time } => self.moved(dx, dy, time),
            Swipe::Ended { cancelled, time } => self.ended(cancelled, time),
        }
        self.0.watchers.repaint();
        self.now()
    }

    fn moved(&self, dx: f64, dy: f64, time: u32) {
        let mut swiped = self.0.swiped.borrow_mut();
        let SwipeState::Tracking(track) = &mut *swiped else {
            return;
        };
        let along = match track.axis {
            Axis::Vertical => -dy,
            Axis::Horizontal => -dx,
        } as f32;
        track.travel += along;
        self.0.samples.borrow_mut().push(time, along);
    }

    fn ended(&self, cancelled: bool, time: u32) {
        let mut samples = self.0.samples.borrow_mut();
        samples.push(time, 0.0);
        let mut swiped = self.0.swiped.borrow_mut();
        let SwipeState::Tracking(track) = &*swiped else {
            return;
        };
        let mut track = track.clone();
        match cancelled {
            true => track.travel = 0.0,
            false => track.speed = samples.speed(),
        }
        *swiped = SwipeState::Released(track);
    }
}
