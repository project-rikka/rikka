use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use rikka_proton_account::{AccountKeys, CoreEvent, Cursors, Feed, FeedError, Poll};

pub(crate) type Feeds<C> = Arc<Vec<Box<dyn Follow<C>>>>;

pub(crate) struct Polled<C> {
    pub(crate) changes: Vec<C>,
    pub(crate) outcome: Result<(), FeedError>,
}

pub(crate) trait Follow<C>: Send + Sync {
    fn apply_core(&self, event: &CoreEvent) -> Result<Option<C>, FeedError>;

    fn poll<'a>(
        &'a self,
        keys: &'a Arc<AccountKeys>,
        cursors: &'a Cursors,
    ) -> Pin<Box<dyn Future<Output = Polled<C>> + Send + 'a>>;
}

pub(crate) struct Followed<F, M> {
    pub(crate) feed: F,
    pub(crate) into: M,
}

impl<C, F, M> Follow<C> for Followed<F, M>
where
    C: Send,
    F: Feed,
    M: Fn(F::Change) -> C + Send + Sync,
{
    fn apply_core(&self, event: &CoreEvent) -> Result<Option<C>, FeedError> {
        Ok(self.feed.apply_core(event)?.map(&self.into))
    }

    fn poll<'a>(
        &'a self,
        keys: &'a Arc<AccountKeys>,
        cursors: &'a Cursors,
    ) -> Pin<Box<dyn Future<Output = Polled<C>> + Send + 'a>> {
        Box::pin(async move {
            let poll = Poll::new(keys, cursors);
            let outcome = self.feed.poll(&poll).await;
            Polled {
                changes: poll.into_changes().into_iter().map(&self.into).collect(),
                outcome,
            }
        })
    }
}
