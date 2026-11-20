use std::fmt;
use std::time::Duration;

use rikka_compositor_reactor::Reactor;
use rikka_compositor_types::OutputId;

use crate::state::Compositor;

pub(crate) struct FrameScheduler {
    reactor: Reactor<Compositor>,
    every: bool,
    named: Vec<OutputId>,
    finished: Vec<OutputId>,
    refreshed: Vec<(OutputId, Duration)>,
    dispatching: bool,
    changed: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FrameTargets<'a> {
    Every,
    Named(&'a [OutputId]),
}

impl FrameScheduler {
    pub(crate) fn new(reactor: &Reactor<Compositor>) -> Self {
        Self {
            reactor: reactor.clone(),
            every: false,
            named: Vec::new(),
            finished: Vec::new(),
            refreshed: Vec::new(),
            dispatching: false,
            changed: true,
        }
    }

    pub(crate) fn request(&mut self, output: OutputId) {
        if !self.named.contains(&output) {
            self.named.push(output);
        }
        self.changed = true;
        self.dispatch_this_turn();
    }

    pub(crate) fn request_all(&mut self) {
        self.every = true;
        self.changed = true;
        self.dispatch_this_turn();
    }

    pub(crate) fn finished(&mut self, output: OutputId) {
        if !self.finished.contains(&output) {
            self.finished.push(output);
        }
        self.dispatch_this_turn();
    }

    pub(crate) fn refreshed(&mut self, output: OutputId, next_presentation: Duration) {
        match self
            .refreshed
            .iter_mut()
            .find(|(refreshed_output, _)| *refreshed_output == output)
        {
            Some((_, next)) => *next = next_presentation,
            None => self.refreshed.push((output, next_presentation)),
        }
        self.dispatch_this_turn();
    }

    pub(crate) fn is_requested(&self, output: OutputId) -> bool {
        self.every || self.named.contains(&output)
    }

    pub(crate) fn input_arrived(&mut self) {
        self.dispatch_this_turn();
    }

    pub(crate) fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    pub(crate) fn next_finished(&mut self) -> Option<OutputId> {
        (!self.finished.is_empty()).then(|| self.finished.remove(0))
    }

    pub(crate) fn next_refreshed(&mut self) -> Option<(OutputId, Duration)> {
        (!self.refreshed.is_empty()).then(|| self.refreshed.remove(0))
    }

    pub(crate) fn take(&mut self) -> Option<FrameTargets<'_>> {
        self.dispatching = false;
        if std::mem::take(&mut self.every) {
            self.named.clear();
            return Some(FrameTargets::Every);
        }
        (!self.named.is_empty()).then_some(FrameTargets::Named(&self.named))
    }

    pub(crate) fn scheduled(&mut self) {
        self.named.clear();
    }

    fn dispatch_this_turn(&mut self) {
        if std::mem::replace(&mut self.dispatching, true) {
            return;
        }
        self.reactor.idle(Compositor::dispatch_frames);
    }
}

impl fmt::Debug for FrameScheduler {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FrameScheduler")
            .field("every", &self.every)
            .field("named", &self.named)
            .finish_non_exhaustive()
    }
}
