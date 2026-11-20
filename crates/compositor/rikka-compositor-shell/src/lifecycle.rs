use std::collections::VecDeque;

use crate::configures::{AckResult, Configures};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Transition {
    InitialCommit,
    Mapped,
    Unmapped,
}

#[derive(Debug, Default)]
pub(crate) struct Applied {
    pub(crate) configured: Option<u32>,
    pub(crate) transition: Option<Transition>,
}

#[derive(Debug)]
struct Latched<C, R> {
    state: R,
    acked: Option<(u32, C)>,
}

#[derive(Debug)]
pub(crate) struct Lifecycle<C, R> {
    sent: Configures<C>,
    acked: Option<(u32, C)>,
    current: Option<(u32, C)>,
    latched: VecDeque<Latched<C, R>>,
    pub(crate) pending: R,
    pub(crate) state: R,
    configure_sent: bool,
    configured: bool,
    initialized: bool,
    mapped: bool,
}

impl<C: Clone, R: Clone> Lifecycle<C, R> {
    pub(crate) fn new(initial: R) -> Self {
        Self {
            sent: Configures::default(),
            acked: None,
            current: None,
            latched: VecDeque::new(),
            pending: initial.clone(),
            state: initial,
            configure_sent: false,
            configured: false,
            initialized: false,
            mapped: false,
        }
    }

    pub(crate) fn sent(&mut self, serial: u32, configure: C) {
        self.sent.push(serial, configure);
        self.configure_sent = true;
    }

    pub(crate) fn ack(&mut self, serial: u32) -> bool {
        match self.sent.ack(serial) {
            AckResult::Configure(configure) => self.acked = Some((serial, configure)),
            AckResult::Dropped => {}
            AckResult::Stale => return true,
            AckResult::Unknown => return false,
        }
        self.configured = true;
        true
    }

    pub(crate) fn latch(&mut self) {
        self.latched.push_back(Latched {
            state: self.pending.clone(),
            acked: self.acked.take(),
        });
    }

    pub(crate) fn apply(&mut self, has_buffer: bool, initial: &R) -> Applied {
        let Some(latched) = self.latched.pop_front() else {
            return Applied::default();
        };
        self.state = latched.state;
        let configured = latched.acked.map(|(serial, configure)| {
            self.current = Some((serial, configure));
            serial
        });
        let transition = self.transition(has_buffer, initial);
        Applied {
            configured,
            transition,
        }
    }

    fn transition(&mut self, has_buffer: bool, initial: &R) -> Option<Transition> {
        if self.mapped && !has_buffer {
            self.reset(initial);
            return Some(Transition::Unmapped);
        }
        if !self.initialized {
            self.initialized = true;
            if !(self.configured && has_buffer) {
                return Some(Transition::InitialCommit);
            }
        }
        if self.configured && has_buffer && !self.mapped {
            self.mapped = true;
            return Some(Transition::Mapped);
        }
        None
    }

    fn reset(&mut self, initial: &R) {
        let sent = std::mem::take(&mut self.sent);
        *self = Self::new(initial.clone());
        self.sent = sent.into_stale();
    }

    pub(crate) fn current(&self) -> Option<&C> {
        self.current.as_ref().map(|(_, configure)| configure)
    }

    pub(crate) fn current_serial(&self) -> Option<u32> {
        self.current.as_ref().map(|(serial, _)| *serial)
    }

    pub(crate) fn latest(&self) -> Option<&C> {
        self.sent
            .last()
            .or(self.acked.as_ref().map(|(_, configure)| configure))
            .or_else(|| self.current())
    }

    pub(crate) fn awaiting_ack(&self) -> bool {
        !self.sent.is_empty()
            || self.acked.is_some()
            || self.latched.iter().any(|latched| latched.acked.is_some())
    }

    pub(crate) fn configure_sent(&self) -> bool {
        self.configure_sent
    }

    pub(crate) fn configured(&self) -> bool {
        self.configured
    }

    pub(crate) fn initialized(&self) -> bool {
        self.initialized
    }

    pub(crate) fn mapped(&self) -> bool {
        self.mapped
    }
}
