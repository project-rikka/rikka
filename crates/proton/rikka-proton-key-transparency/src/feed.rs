use rikka_proton_account::{Feed, FeedError, FeedFuture, Poll};

use crate::audit::AuditReport;
use crate::client::KeyTransparency;
use crate::error::Error;

impl Feed for KeyTransparency {
    type Change = Box<AuditReport>;

    fn poll<'a>(&'a self, poll: &'a Poll<'a, Box<AuditReport>>) -> FeedFuture<'a> {
        Box::pin(async move {
            if !self.is_audit_due() {
                return Ok(());
            }
            if let Err(error) = self.refresh_log_list().await {
                tracing::warn!(%error, "failed to refresh CT log list");
            }
            if !self.is_available().await {
                return Ok(());
            }
            let report = self.audit(poll.keys()).await?;
            poll.emit(Box::new(report));
            Ok(())
        })
    }
}

impl From<Error> for FeedError {
    fn from(error: Error) -> FeedError {
        FeedError::new(error, Error::api)
    }
}
