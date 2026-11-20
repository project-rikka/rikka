use rikka_sync::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rikka_types::CancelToken;
use tokio::sync::mpsc::UnboundedSender;

use super::event::Event;
use crate::error::{Error, Result};
use crate::hid::{CancelSignal, Hotplug};

#[derive(Debug, Clone)]
pub(crate) struct Watch {
    cancel: CancelToken,
    deadline: Instant,
    events: Events,
}

impl Watch {
    pub(crate) fn new(cancel: CancelToken, deadline: Instant, events: Events) -> Watch {
        Watch {
            cancel,
            deadline,
            events,
        }
    }

    pub(crate) fn bounded(timeout: Duration) -> Watch {
        Watch::new(
            CancelToken::new(),
            Instant::now() + timeout,
            Events::disabled(),
        )
    }

    pub(crate) fn fork(&self) -> (Watch, CancelToken) {
        let cancel = self.cancel.child();
        let watch = Watch {
            cancel: cancel.clone(),
            deadline: self.deadline,
            events: Events::disabled(),
        };
        (watch, cancel)
    }

    pub(crate) fn stop_reason(&self) -> Option<Error> {
        if self.cancel.is_cancelled() {
            Some(Error::Cancelled)
        } else if Instant::now() >= self.deadline {
            Some(Error::Timeout)
        } else {
            None
        }
    }

    pub(crate) fn check(&self) -> Result<()> {
        self.stop_reason().map_or(Ok(()), Err)
    }

    pub(crate) fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    pub(crate) fn cancel_signal(&self) -> Result<CancelSignal<'_>> {
        CancelSignal::new(&self.cancel).map_err(Error::io)
    }

    pub(crate) fn pause(&self, length: Duration) -> Result<()> {
        let until = (Instant::now() + length).min(self.deadline);
        let cancelled = self
            .cancel
            .wait_timeout(until.saturating_duration_since(Instant::now()));
        match cancelled {
            true => Err(Error::Cancelled),
            false => self.check(),
        }
    }

    pub(crate) fn wait_for_hotplug(&self, hotplug: &mut dyn Hotplug) -> Result<()> {
        hotplug
            .wait(
                &self.cancel,
                self.deadline.saturating_duration_since(Instant::now()),
            )
            .map_err(Error::io)?;
        self.check()
    }

    pub(crate) fn emit(&self, event: Event) {
        self.events.emit(event);
    }

    pub(crate) fn touch(&self) {
        self.events.emit(Event::Touch);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Events {
    sender: Option<UnboundedSender<Event>>,
    last: Arc<Mutex<Option<Event>>>,
}

impl Events {
    pub(crate) fn new(sender: UnboundedSender<Event>) -> Events {
        Events {
            sender: Some(sender),
            last: Arc::default(),
        }
    }

    pub(crate) fn disabled() -> Events {
        Events {
            sender: None,
            last: Arc::default(),
        }
    }

    fn emit(&self, event: Event) {
        let Some(sender) = &self.sender else {
            return;
        };

        let mut last = self.last.lock();
        if event.repeats() && last.as_ref() == Some(&event) {
            return;
        }

        *last = Some(event.clone());
        if sender.send(event).is_err() {
            tracing::debug!("ceremony event receiver dropped");
        }
    }
}
