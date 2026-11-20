use std::time::{Duration, SystemTime};

use http::{HeaderMap, Method, header};
use rand::Rng;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    max_retries: u32,
    initial_delay: Duration,
    max_delay: Duration,
    multiplier: f64,
    jitter: f64,
    respect_retry_after: bool,
    max_retry_after: Duration,
    non_idempotent: bool,
}

impl RetryPolicy {
    #[must_use]
    pub fn new() -> RetryPolicy {
        RetryPolicy::default()
    }

    #[must_use]
    pub fn never() -> RetryPolicy {
        RetryPolicy {
            max_retries: 0,
            ..RetryPolicy::default()
        }
    }

    #[must_use]
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    #[must_use]
    pub fn initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    #[must_use]
    pub fn max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    #[must_use]
    pub fn multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier.max(1.0);
        self
    }

    #[must_use]
    pub fn jitter(mut self, jitter: f64) -> Self {
        self.jitter = jitter.clamp(0.0, 1.0);
        self
    }

    #[must_use]
    pub fn respect_retry_after(mut self, respect: bool) -> Self {
        self.respect_retry_after = respect;
        self
    }

    #[must_use]
    pub fn max_retry_after(mut self, limit: Duration) -> Self {
        self.max_retry_after = limit;
        self
    }

    #[must_use]
    pub fn non_idempotent(mut self, retry: bool) -> Self {
        self.non_idempotent = retry;
        self
    }

    pub(crate) fn allows(&self, method: &Method) -> bool {
        if self.max_retries == 0 {
            return false;
        }
        self.non_idempotent || method.is_idempotent()
    }

    pub(crate) fn delay(&self, retry: u32, hint: Option<Duration>) -> Option<Duration> {
        if retry >= self.max_retries {
            return None;
        }

        if let Some(hint) = hint.filter(|_| self.respect_retry_after) {
            return (hint <= self.max_retry_after).then_some(hint);
        }

        let exponent = i32::try_from(retry).unwrap_or(i32::MAX);
        let seconds = self.initial_delay.as_secs_f64() * self.multiplier.powi(exponent);
        let backoff = Duration::try_from_secs_f64(seconds)
            .unwrap_or(self.max_delay)
            .min(self.max_delay);

        Some(self.apply_jitter(backoff))
    }

    fn apply_jitter(&self, backoff: Duration) -> Duration {
        if self.jitter <= f64::EPSILON {
            return backoff;
        }
        let factor = 1.0 - self.jitter * rand::rng().random_range(0.0..1.0);
        backoff.mul_f64(factor)
    }
}

impl Default for RetryPolicy {
    fn default() -> RetryPolicy {
        RetryPolicy {
            max_retries: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
            jitter: 0.5,
            respect_retry_after: true,
            max_retry_after: Duration::from_secs(60),
            non_idempotent: false,
        }
    }
}

pub(crate) fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    let value = headers.get(header::RETRY_AFTER)?.to_str().ok()?.trim();

    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }

    let deadline = jiff::fmt::rfc2822::DateTimeParser::new()
        .parse_timestamp(value)
        .ok()?;
    Some(
        SystemTime::from(deadline)
            .duration_since(SystemTime::now())
            .unwrap_or(Duration::ZERO),
    )
}
