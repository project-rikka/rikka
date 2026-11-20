use std::future::{self, Future};
use std::pin::Pin;

use crate::core_events::CoreEvent;
use crate::feed_error::FeedError;
use crate::poll::Poll;

pub type FeedFuture<'a> = Pin<Box<dyn Future<Output = Result<(), FeedError>> + Send + 'a>>;

pub trait Feed: Send + Sync + 'static {
    type Change: Send + 'static;

    fn apply_core(&self, _event: &CoreEvent) -> Result<Option<Self::Change>, FeedError> {
        Ok(None)
    }

    fn poll<'a>(&'a self, _poll: &'a Poll<'a, Self::Change>) -> FeedFuture<'a> {
        Box::pin(future::ready(Ok(())))
    }
}

pub trait EventPage {
    fn next_id(&self) -> &str;

    fn has_more(&self) -> bool;
}
