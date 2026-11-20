use std::time::{Duration, Instant};

use rikka_compositor_reactor::Token;
use rikka_seam::Responder;
use rikka_session::agent::{Idle, WaitIdleRequest};

#[derive(Debug, Default)]
pub(crate) struct IdleWaits {
    waits: Vec<IdleWait>,
    frame_in_flight: bool,
    last_frame: Option<Instant>,
    timer: Option<Token>,
}

#[derive(Debug)]
struct IdleWait {
    responder: Responder,
    quiet: Duration,
    since: Instant,
    deadline: Instant,
}

impl IdleWaits {
    pub(crate) fn add(&mut self, responder: Responder, request: WaitIdleRequest, now: Instant) {
        self.waits.push(IdleWait {
            responder,
            quiet: request.quiet(),
            since: now,
            deadline: now + request.timeout(),
        });
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.waits.is_empty()
    }

    pub(crate) fn frame_composed(&mut self, now: Instant) {
        self.frame_in_flight = true;
        self.last_frame = Some(now);
    }

    pub(crate) fn frame_finished(&mut self, now: Instant) {
        self.frame_in_flight = false;
        self.last_frame = Some(now);
    }

    pub(crate) fn replace_timer(&mut self, timer: Option<Token>) -> Option<Token> {
        std::mem::replace(&mut self.timer, timer)
    }

    pub(crate) fn resolve(&mut self, now: Instant, frame_requested: bool) -> Option<Instant> {
        let busy = frame_requested || self.frame_in_flight;
        let last_frame = self.last_frame;
        let mut next_check: Option<Instant> = None;
        for wait in std::mem::take(&mut self.waits) {
            let quiet_since = last_frame.map_or(wait.since, |frame| frame.max(wait.since));
            let quiet_until = quiet_since + wait.quiet;
            if now >= wait.deadline {
                wait.responder.encode(&Idle::TimedOut);
            } else if !busy && now >= quiet_until {
                wait.responder.encode(&Idle::Quiescent);
            } else {
                let due = match busy {
                    true => wait.deadline,
                    false => wait.deadline.min(quiet_until),
                };
                next_check = Some(next_check.map_or(due, |next| next.min(due)));
                self.waits.push(wait);
            }
        }
        next_check
    }
}
