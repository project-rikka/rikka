use std::time::{Duration, Instant};

use rikka_compositor_reactor::Reactor;
use rikka_renderer::TRIM_AFTER;

use crate::backend::Backend;
use crate::state::Compositor;

#[derive(Debug)]
pub(crate) struct Resting {
    reactor: Reactor<Compositor>,
    drew: Option<Instant>,
    armed: bool,
}

impl Resting {
    pub(crate) fn new(reactor: &Reactor<Compositor>) -> Self {
        Self {
            reactor: reactor.clone(),
            drew: None,
            armed: false,
        }
    }

    pub(crate) fn drew(&mut self) {
        self.drew = Some(Instant::now());
        if !self.armed {
            self.arm(TRIM_AFTER);
        }
    }

    fn arm(&mut self, after: Duration) {
        self.armed = true;
        self.reactor.after(after, |compositor: &mut Compositor| {
            compositor.resting.rest_when_quiet(&mut *compositor.backend)
        });
    }

    fn remaining_idle(&self) -> Option<Duration> {
        let drew = self.drew?;
        Some(TRIM_AFTER.saturating_sub(drew.elapsed()))
    }

    fn rest_when_quiet(&mut self, backend: &mut dyn Backend) {
        self.armed = false;
        match self.remaining_idle() {
            Some(left) if !left.is_zero() => self.arm(left),
            Some(_) => {
                self.drew = None;
                backend.rest();
            }
            None => {}
        }
    }
}
