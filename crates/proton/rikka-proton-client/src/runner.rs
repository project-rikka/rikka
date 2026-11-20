use std::sync::Arc;
use std::time::Duration;

use futures_util::future::join_all;
use rikka_proton_account::{CoreEvent, Cursors, EventId, FeedError, Poll};
use rikka_proton_api::{delay, offload};
use rikka_sync::Mutex;
use tokio::sync::mpsc;

use crate::event_loop::Change;
use crate::follow::{Feeds, Polled};
use crate::renewal::Renewal;
use crate::session::Session;

const MAX_BACKOFF: Duration = Duration::from_secs(10 * 60);
const CORE_CURSOR: &str = "core";
const JITTER: f64 = 0.1;

enum Stop {
    Closed,
    Failed(FeedError),
}

impl From<FeedError> for Stop {
    fn from(error: FeedError) -> Stop {
        Stop::Failed(error)
    }
}

pub(crate) struct Runner<C> {
    session: Mutex<Session>,
    cursors: Cursors,
    sender: mpsc::Sender<Change<C>>,
    interval: Duration,
    feeds: Feeds<C>,
}

impl<C: Send + 'static> Runner<C> {
    pub(crate) fn new(
        session: Session,
        sender: mpsc::Sender<Change<C>>,
        interval: Duration,
        feeds: Feeds<C>,
    ) -> Runner<C> {
        Runner {
            cursors: Cursors::for_account(session.proton().account()),
            session: Mutex::new(session),
            sender,
            interval,
            feeds,
        }
    }

    pub(crate) async fn run(self: Box<Self>) {
        let mut failures = 0;
        loop {
            match self.tick().await {
                Ok(()) => failures = 0,
                Err(Stop::Closed) => return,
                Err(Stop::Failed(error)) if self.is_signed_out(&error) => {
                    drop(self.sender.send(Change::SignedOut).await);
                    return;
                }
                Err(Stop::Failed(error)) => {
                    tracing::warn!(%error, "failed to poll events, backing off");
                    failures += 1;
                }
            }
            let wait = delay::doubling(self.interval, failures, MAX_BACKOFF.max(self.interval));
            tokio::time::sleep(delay::jittered(wait, JITTER)).await;
            if self.sender.is_closed() {
                return;
            }
        }
    }

    async fn tick(&self) -> Result<(), Stop> {
        Box::pin(self.core()).await?;
        let session = self.session();
        let polls = self
            .feeds
            .iter()
            .map(|feed| feed.poll(session.keys(), &self.cursors));
        for Polled { changes, outcome } in join_all(polls).await {
            for change in changes {
                self.send(Change::Product(change)).await?;
            }
            match outcome {
                Ok(()) => {}
                Err(error) if self.is_signed_out(&error) => return Err(Stop::Failed(error)),
                Err(error) => tracing::debug!(%error, "failed to poll product events"),
            }
        }
        Ok(())
    }

    async fn core(&self) -> Result<(), Stop> {
        let session = self.session();
        let account = session.proton().account();
        let poll = Poll::new(session.keys(), &self.cursors);
        let walked = poll
            .walk(
                CORE_CURSOR,
                async { Ok(account.latest_event().await?.as_str().to_owned()) },
                |since| async move {
                    Ok(account
                        .core_events::<CoreEvent>(&EventId::from(since.as_str()))
                        .await?)
                },
                |_, event: CoreEvent| self.apply_core(&poll, event),
            )
            .await;
        for change in poll.into_changes() {
            self.send(change).await?;
        }
        Ok(walked?)
    }

    async fn apply_core(
        &self,
        poll: &Poll<'_, Change<C>>,
        event: CoreEvent,
    ) -> Result<(), FeedError> {
        let feeds = Arc::clone(&self.feeds);
        let (event, applied) = offload(move || {
            let applied: Result<Vec<C>, FeedError> = feeds
                .iter()
                .filter_map(|feed| feed.apply_core(&event).transpose())
                .collect();
            (event, applied)
        })
        .await;
        for change in applied? {
            poll.emit(Change::Product(change));
        }
        let refresh = event.refresh();
        if !refresh.is_none() {
            poll.emit(Change::Refreshed(refresh));
        }
        if event.touches_account() || refresh.includes_everything() {
            let renewal = self.session().renew().await?;
            match &renewal {
                Renewal::Unchanged => return Ok(()),
                Renewal::Updated(session) | Renewal::Rekeyed(session) => {
                    *self.session.lock() = (**session).clone();
                }
            }
            poll.emit(Change::Renewed(renewal));
        }
        Ok(())
    }

    async fn send(&self, change: Change<C>) -> Result<(), Stop> {
        self.sender.send(change).await.map_err(|_| Stop::Closed)
    }

    fn session(&self) -> Session {
        self.session.lock().clone()
    }

    fn is_signed_out(&self, error: &FeedError) -> bool {
        error.is_signed_out() || self.session().proton().account().api().session().is_none()
    }
}
