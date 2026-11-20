use std::time::Duration;

use rikka_compositor_types::{Gesture, OutputId};
use rikka_session::gestures::{Axis, FINGERS, Swipe};

const DECIDED: f64 = 12.0;

#[derive(Debug, Default)]
pub struct Swiping {
    state: State,
}

#[derive(Debug, Default)]
enum State {
    #[default]
    Idle,
    Deciding {
        gestures: Vec<Gesture>,
        travel: (f64, f64),
    },
    Kept,
    Scrolling,
    Passed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scroll {
    Began { output: OutputId },
    Moved { dx: f64, time: Duration },
    Ended { time: Duration },
}

#[derive(Debug, Default)]
pub struct Outcome {
    pub announced: Vec<Swipe>,
    pub passed: Vec<Gesture>,
    pub scrolled: Vec<Scroll>,
}

#[derive(Debug)]
pub struct Pointed {
    pub announcing: bool,
    pub sideways_kept: bool,
    pub scrolls_on: Option<OutputId>,
    pub output: Option<String>,
}

impl Swiping {
    pub fn handle(&mut self, gesture: Gesture, pointed: impl FnOnce() -> Pointed) -> Outcome {
        let mut outcome = Outcome::default();
        let state = std::mem::take(&mut self.state);
        self.state = match (state, gesture) {
            (state, Gesture::SwipeBegin { fingers, time }) => {
                abandon(state, time, &mut outcome);
                match fingers == FINGERS {
                    true => State::Deciding {
                        gestures: vec![gesture],
                        travel: (0.0, 0.0),
                    },
                    false => {
                        outcome.passed.push(gesture);
                        State::Passed
                    }
                }
            }
            (
                State::Deciding {
                    mut gestures,
                    travel: (x, y),
                },
                Gesture::SwipeUpdate { delta, time },
            ) => {
                gestures.push(gesture);
                let travel = (x + delta.x, y + delta.y);
                match travel.0.hypot(travel.1) < DECIDED {
                    true => State::Deciding { gestures, travel },
                    false => decide(gestures, travel, time, pointed(), &mut outcome),
                }
            }
            (State::Deciding { mut gestures, .. }, Gesture::SwipeEnd { .. }) => {
                gestures.push(gesture);
                outcome.passed = gestures;
                State::Idle
            }
            (State::Kept, Gesture::SwipeUpdate { delta, time }) => {
                outcome.announced.push(Swipe::Moved {
                    dx: delta.x,
                    dy: delta.y,
                    time: millis(time),
                });
                State::Kept
            }
            (State::Kept, Gesture::SwipeEnd { cancelled, time }) => {
                outcome.announced.push(Swipe::Ended {
                    cancelled,
                    time: millis(time),
                });
                State::Idle
            }
            (State::Scrolling, Gesture::SwipeUpdate { delta, time }) => {
                outcome.scrolled.push(Scroll::Moved { dx: delta.x, time });
                State::Scrolling
            }
            (State::Scrolling, Gesture::SwipeEnd { time, .. }) => {
                outcome.scrolled.push(Scroll::Ended { time });
                State::Idle
            }
            (State::Passed, Gesture::SwipeEnd { .. }) => {
                outcome.passed.push(gesture);
                State::Idle
            }
            (state, _) => {
                outcome.passed.push(gesture);
                state
            }
        };
        outcome
    }
}

fn abandon(state: State, time: Duration, outcome: &mut Outcome) {
    match state {
        State::Deciding { gestures, .. } => outcome.passed = gestures,
        State::Kept => outcome.announced.push(Swipe::Ended {
            cancelled: true,
            time: millis(time),
        }),
        State::Scrolling => outcome.scrolled.push(Scroll::Ended { time }),
        State::Idle | State::Passed => {}
    }
}

fn decide(
    gestures: Vec<Gesture>,
    travel: (f64, f64),
    time: Duration,
    pointed: Pointed,
    outcome: &mut Outcome,
) -> State {
    let axis = match travel.1.abs() >= travel.0.abs() {
        true => Axis::Vertical,
        false => Axis::Horizontal,
    };
    let announced = pointed.announcing && (axis == Axis::Vertical || pointed.sideways_kept);
    if !announced {
        if axis == Axis::Horizontal
            && let Some(output) = pointed.scrolls_on
        {
            outcome.scrolled.push(Scroll::Began { output });
            outcome.scrolled.push(Scroll::Moved { dx: travel.0, time });
            return State::Scrolling;
        }
        outcome.passed = gestures;
        return State::Passed;
    }
    outcome.announced.push(Swipe::Began {
        axis,
        output: pointed.output,
    });
    outcome.announced.push(Swipe::Moved {
        dx: travel.0,
        dy: travel.1,
        time: millis(time),
    });
    State::Kept
}

fn millis(time: Duration) -> u32 {
    (time.as_millis() % (u128::from(u32::MAX) + 1)) as u32
}
