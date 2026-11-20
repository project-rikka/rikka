use std::future::Future;
use std::sync::Arc;

use rikka_sync::Mutex;

use crate::account_keys::AccountKeys;
use crate::cursors::Cursors;
use crate::feed::EventPage;
use crate::feed_error::FeedError;

#[derive(Debug)]
pub struct Poll<'a, C> {
    keys: &'a Arc<AccountKeys>,
    cursors: &'a Cursors,
    changes: Mutex<Vec<C>>,
}

impl<'a, C> Poll<'a, C> {
    #[must_use]
    pub fn new(keys: &'a Arc<AccountKeys>, cursors: &'a Cursors) -> Poll<'a, C> {
        Poll {
            keys,
            cursors,
            changes: Mutex::new(Vec::new()),
        }
    }

    #[must_use]
    pub fn keys(&self) -> &Arc<AccountKeys> {
        self.keys
    }

    pub fn emit(&self, change: C) {
        self.changes.lock().push(change);
    }

    #[must_use]
    pub fn into_changes(self) -> Vec<C> {
        self.changes.into_inner()
    }

    pub async fn walk<P, Latest, Fetch, Fetched, Apply, Applied>(
        &self,
        cursor: &str,
        latest: Latest,
        fetch: Fetch,
        apply: Apply,
    ) -> Result<(), FeedError>
    where
        P: EventPage,
        Latest: Future<Output = Result<String, FeedError>>,
        Fetch: Fn(String) -> Fetched,
        Fetched: Future<Output = Result<P, FeedError>>,
        Apply: Fn(String, P) -> Applied,
        Applied: Future<Output = Result<(), FeedError>>,
    {
        let Some(mut since) = self.cursors.get(cursor) else {
            let latest = latest.await?;
            self.cursors.set(cursor, &latest);
            return Ok(());
        };
        loop {
            let page = fetch(since.clone()).await?;
            let (next, more) = (page.next_id().to_owned(), page.has_more());
            apply(since.clone(), page).await?;
            if next.is_empty() || next == since {
                return Ok(());
            }
            self.cursors.set(cursor, &next);
            since = next;
            if !more {
                return Ok(());
            }
        }
    }
}
