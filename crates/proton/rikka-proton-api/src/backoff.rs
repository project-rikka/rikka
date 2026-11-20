use std::time::Duration;

use crate::delay::random_unit;
use http::StatusCode;

pub(crate) const BASE_DELAY: Duration = Duration::from_millis(500);
pub(crate) const DEFAULT_RETRIES: u32 = 2;

const MAX_RETRY_AFTER: Duration = Duration::from_secs(10);
const CONNECTION_FACTOR: f64 = 2.0;
const RETRY_AFTER_FACTOR: f64 = 1.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Attempt {
    ConnectionFailed,
    Status(StatusCode),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Backoff {
    retries: u32,
    base: Duration,
}

impl Backoff {
    pub(crate) fn new(retries: u32) -> Backoff {
        Backoff {
            retries,
            base: BASE_DELAY,
        }
    }

    pub(crate) fn should_retry(
        &self,
        attempt: u32,
        result: Attempt,
        retry_after: Option<Duration>,
    ) -> bool {
        if attempt >= self.retries || !is_retryable(result) {
            return false;
        }

        if result == Attempt::Status(StatusCode::REQUEST_TIMEOUT) {
            return attempt == 0;
        }

        match retry_after {
            Some(after) => after <= MAX_RETRY_AFTER,
            None => !matches!(
                result,
                Attempt::Status(StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE)
            ),
        }
    }

    pub(crate) fn delay(&self, attempt: u32, retry_after: Option<Duration>) -> Duration {
        match retry_after {
            Some(after) => jittered(after, attempt, RETRY_AFTER_FACTOR),
            None => jittered(self.base, attempt, CONNECTION_FACTOR),
        }
    }
}

fn is_retryable(result: Attempt) -> bool {
    match result {
        Attempt::ConnectionFailed => true,
        Attempt::Status(status) => {
            status.is_server_error()
                || matches!(
                    status,
                    StatusCode::MISDIRECTED_REQUEST
                        | StatusCode::REQUEST_TIMEOUT
                        | StatusCode::TOO_MANY_REQUESTS
                )
        }
    }
}

fn jittered(base: Duration, attempt: u32, factor: f64) -> Duration {
    let exponent = i32::try_from(attempt).unwrap_or(i32::MAX);
    let low = factor.powi(exponent);
    let high = factor.powi(exponent.saturating_add(1));

    base.mul_f64(low + (high - low) * random_unit())
}
