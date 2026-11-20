mod clock;

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use cut_project::Rate;
use cut_render::Playing;
use rikka_ui::Observers;

use clock::Clock;

const SHUTTLE: [f64; 4] = [1.0, 2.0, 4.0, 8.0];
const PLAY_START_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
    Forward,
    Backward,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Transport(Rc<Inner>);

#[derive(Debug)]
struct Inner {
    clock: Cell<Clock>,
    rate: Cell<Rate>,
    position: Observers,
    state: Observers,
}

impl Default for Inner {
    fn default() -> Inner {
        Inner {
            clock: Cell::new(Clock::default()),
            rate: Cell::new(Rate::VIDEO),
            position: Observers::new(),
            state: Observers::new(),
        }
    }
}

impl PartialEq for Transport {
    fn eq(&self, other: &Transport) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Transport {
    fn clock(&self) -> Clock {
        self.0.clock.get()
    }

    fn per_second(&self) -> f64 {
        self.0.rate.get().per_second()
    }

    fn change(&self, edit: impl FnOnce(&mut Clock, f64)) {
        let mut clock = self.clock();
        edit(&mut clock, self.per_second());
        self.0.clock.set(clock);
    }

    pub(crate) fn rate(&self) -> Rate {
        self.0.state.read();
        self.0.rate.get()
    }

    pub(crate) fn set_rate(&self, rate: Rate) {
        if self.0.rate.get() == rate {
            return;
        }
        let now = Instant::now();
        let frame = self.clock().frame(now, self.per_second());
        self.0.rate.set(rate);
        self.change(|clock, _| clock.seek(frame, now));
        self.0.state.changed();
        self.0.position.changed();
    }

    pub(crate) fn exact(&self, now: Instant) -> f64 {
        self.0.position.read();
        self.clock().frame(now, self.per_second())
    }

    pub(crate) fn frame(&self) -> i64 {
        self.exact(Instant::now()).floor() as i64
    }

    pub(crate) fn speed(&self) -> Option<f64> {
        self.0.state.read();
        self.clock().speed()
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.speed().is_some()
    }

    pub(crate) fn playing(&self) -> Option<Playing> {
        self.0.state.read();
        self.0.position.read();
        self.clock().running()
    }

    pub(crate) fn is_running(&self) -> bool {
        self.0.state.read();
        self.clock().is_running()
    }

    pub(crate) fn has_waited(&self, now: Instant) -> bool {
        self.clock()
            .asked_at()
            .is_some_and(|requested| now.saturating_duration_since(requested) >= PLAY_START_TIMEOUT)
    }

    pub(crate) fn play(&self, speed: f64) {
        let now = Instant::now();
        self.change(|clock, per_second| clock.ask(speed, now, per_second));
        self.0.state.changed();
    }

    pub(crate) fn shuttle(&self, direction: Direction) {
        let sign = match direction {
            Direction::Forward => 1.0,
            Direction::Backward => -1.0,
        };
        let faster = match self.clock().speed() {
            Some(speed) if speed * sign > 0.0 => SHUTTLE
                .into_iter()
                .find(|step| *step > speed.abs())
                .unwrap_or(SHUTTLE[SHUTTLE.len() - 1]),
            _ => SHUTTLE[0],
        };
        self.play(faster * sign);
    }

    pub(crate) fn run(&self, now: Instant) {
        self.change(|clock, _| clock.run(now));
        self.0.state.changed();
        self.0.position.changed();
    }

    pub(crate) fn stop(&self) {
        if self.clock().speed().is_none() {
            return;
        }
        let now = Instant::now();
        self.change(|clock, per_second| clock.stop(now, per_second));
        self.0.state.changed();
        self.0.position.changed();
    }

    pub(crate) fn seek(&self, frame: i64) {
        let clock = self.clock();
        if !clock.is_running() && clock.frame(Instant::now(), self.per_second()) == frame as f64 {
            return;
        }
        let now = Instant::now();
        self.change(|clock, _| clock.seek(frame as f64, now));
        self.0.position.changed();
    }

    pub(crate) fn halt_at(&self, frame: i64) {
        self.stop();
        self.seek(frame);
    }
}
