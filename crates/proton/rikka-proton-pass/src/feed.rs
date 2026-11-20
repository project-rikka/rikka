use futures_util::future::join_all;
use rikka_proton_account::{EventPage, Feed, FeedError, FeedFuture, Poll};

use crate::api::UserEvents;
use crate::client::PassClient;
use crate::error::Error;
use crate::identifier::{EventId, ShareId};

const USER_CURSOR: &str = "pass";

impl Feed for PassClient {
    type Change = Box<UserEvents>;

    fn poll<'a>(&'a self, poll: &'a Poll<'a, Box<UserEvents>>) -> FeedFuture<'a> {
        Box::pin(async move {
            let pass = self.clone().with_keys(poll.keys())?;
            let pass = &pass;
            poll.walk(
                USER_CURSOR,
                async { Ok(pass.latest_user_event().await?.as_str().to_owned()) },
                |since| async move { Ok(pass.user_events(&EventId::from(since.as_str())).await?) },
                |_, events: UserEvents| async move {
                    let shares: Vec<ShareId> = match events.is_full_refresh() {
                        true => pass
                            .shares()
                            .await?
                            .iter()
                            .map(|share| share.id().clone())
                            .collect(),
                        false => events.changed_shares(),
                    };
                    let synced = join_all(shares.iter().map(|share| pass.sync_items(share))).await;
                    for outcome in synced {
                        if let Err(error) = outcome {
                            tracing::debug!(%error, "failed to sync vault");
                        }
                    }
                    if events.is_full_refresh() || !shares.is_empty() || events.refreshes_user() {
                        poll.emit(Box::new(events));
                    }
                    Ok(())
                },
            )
            .await
        })
    }
}

impl EventPage for UserEvents {
    fn next_id(&self) -> &str {
        self.last_event().as_str()
    }

    fn has_more(&self) -> bool {
        self.is_pending()
    }
}

impl From<Error> for FeedError {
    fn from(error: Error) -> FeedError {
        FeedError::new(error, Error::api)
    }
}
