use std::time::{Duration, Instant};

const EXPIRY_MARGIN: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct Token {
    value: String,
    expires: Instant,
}

#[derive(Clone, Debug)]
pub enum Progress {
    Challenged,
    Granted(Token),
}

impl Token {
    pub(crate) fn new(value: String, lifetime: Duration) -> Token {
        Token {
            value,
            expires: Instant::now() + lifetime,
        }
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub fn is_fresh(&self) -> bool {
        Instant::now() + EXPIRY_MARGIN < self.expires
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Token")
            .field("expires", &self.expires)
            .finish_non_exhaustive()
    }
}

pub(crate) fn lifetime(seconds: i32, fallback: Duration) -> Duration {
    u64::try_from(seconds)
        .ok()
        .filter(|seconds| *seconds > 0)
        .map_or(fallback, Duration::from_secs)
}
