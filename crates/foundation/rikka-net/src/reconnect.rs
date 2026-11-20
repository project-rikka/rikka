use std::time::Duration;

use crate::cancel::{self, CancelToken};
use crate::error::{Error, Result};
use crate::retry::RetryPolicy;

#[derive(Debug)]
pub(crate) struct Reconnect {
    policy: Option<RetryPolicy>,
    attempts: u32,
    retry_after: Option<Duration>,
}

impl Reconnect {
    pub(crate) fn new(policy: Option<RetryPolicy>) -> Reconnect {
        Reconnect {
            policy,
            attempts: 0,
            retry_after: None,
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.policy.is_some()
    }

    pub(crate) async fn wait(&mut self, cancel: Option<&CancelToken>) -> Result<bool> {
        let Some(delay) = self.delay() else {
            return Ok(false);
        };

        cancel::sleep(delay, cancel).await?;
        Ok(true)
    }

    pub(crate) fn succeeded(&mut self) {
        self.attempts = 0;
        self.retry_after = None;
    }

    pub(crate) fn failed(&mut self, error: &Error) -> bool {
        if !error.is_transient() {
            return false;
        }

        self.attempts += 1;
        self.retry_after = error.retry_after();
        true
    }

    pub(crate) fn attempts(&self) -> u32 {
        self.attempts
    }

    fn delay(&self) -> Option<Duration> {
        self.policy.as_ref()?.delay(self.attempts, self.retry_after)
    }
}
