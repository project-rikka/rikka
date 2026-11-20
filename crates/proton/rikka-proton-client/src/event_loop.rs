use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use rikka_proton_account::{Feed, Refresh};
use tokio::sync::mpsc;

use crate::follow::{Follow, Followed};
use crate::renewal::Renewal;
use crate::runner::Runner;
use crate::session::Session;
use crate::watch::Watch;

const DEFAULT_INTERVAL: Duration = Duration::from_secs(30);
const MIN_INTERVAL: Duration = Duration::from_millis(1);
const CHANGE_CAPACITY: usize = 64;

#[derive(Debug)]
pub enum Change<C> {
    Product(C),
    Refreshed(Refresh),
    Renewed(Renewal),
    SignedOut,
}

#[must_use]
pub struct EventLoop<C> {
    session: Session,
    interval: Duration,
    feeds: Vec<Box<dyn Follow<C>>>,
}

impl<C: Send + 'static> EventLoop<C> {
    pub(crate) fn new(session: Session) -> EventLoop<C> {
        EventLoop {
            session,
            interval: DEFAULT_INTERVAL,
            feeds: Vec::new(),
        }
    }

    pub fn every(mut self, interval: Duration) -> EventLoop<C> {
        self.interval = interval.max(MIN_INTERVAL);
        self
    }

    pub fn feed<F: Feed>(
        mut self,
        feed: F,
        into: impl Fn(F::Change) -> C + Send + Sync + 'static,
    ) -> EventLoop<C> {
        self.feeds.push(Box::new(Followed { feed, into }));
        self
    }

    pub fn start(self) -> Watch<C> {
        let (sender, changes) = mpsc::channel(CHANGE_CAPACITY);
        let runner = Runner::new(self.session, sender, self.interval, Arc::new(self.feeds));
        Watch::new(changes, runner)
    }
}

impl<C> fmt::Debug for EventLoop<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EventLoop")
            .field("interval", &self.interval)
            .field("feeds", &self.feeds.len())
            .finish_non_exhaustive()
    }
}
