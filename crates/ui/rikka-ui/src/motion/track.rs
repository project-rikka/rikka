use std::time::{Duration, Instant};

use rikka_animation::{Channel, Presence};

use crate::motion::Clock;

pub(crate) trait Course {
    fn is_resting(&self) -> bool;
}

impl Course for [Channel; 4] {
    fn is_resting(&self) -> bool {
        self.iter().all(Channel::is_resting)
    }
}

impl Course for Presence {
    fn is_resting(&self) -> bool {
        Presence::is_resting(self)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Track<C> {
    pub(crate) course: C,
    last: Option<Instant>,
}

impl<C: Course> Track<C> {
    pub(crate) fn new(course: C, last: Option<Instant>) -> Track<C> {
        Track { course, last }
    }

    pub(crate) fn last(&self) -> Option<Instant> {
        self.last
    }

    pub(crate) fn wake(&mut self, clock: Clock) {
        if self.course.is_resting() {
            self.last = Some(clock.began);
        }
    }

    pub(crate) fn elapsed(&mut self, clock: Clock) -> Duration {
        let since = match (self.course.is_resting(), self.last) {
            (false, Some(last)) => last,
            _ => clock.began,
        };
        self.last = Some(clock.now);
        clock.now.saturating_duration_since(since)
    }
}
