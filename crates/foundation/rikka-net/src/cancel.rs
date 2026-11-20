use std::future::Future;
use std::time::Duration;

pub use rikka_types::CancelToken;

use crate::error::{Error, Result};

pub(crate) trait CancelGuard {
    fn guard(&self) -> Result<()>;
}

impl CancelGuard for CancelToken {
    fn guard(&self) -> Result<()> {
        if self.is_cancelled() {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
}

pub(crate) async fn until_cancelled<F: Future>(
    future: F,
    cancel: Option<&CancelToken>,
) -> Result<F::Output> {
    let Some(token) = cancel else {
        return Ok(future.await);
    };

    tokio::select! {
        biased;
        () = token.cancelled() => Err(Error::Cancelled),
        output = future => Ok(output),
    }
}

pub(crate) async fn pending_until_cancelled(cancel: Option<&CancelToken>) {
    match cancel {
        Some(token) => token.cancelled().await,
        None => std::future::pending().await,
    }
}

pub(crate) async fn sleep(delay: Duration, cancel: Option<&CancelToken>) -> Result<()> {
    until_cancelled(tokio::time::sleep(delay), cancel).await
}
