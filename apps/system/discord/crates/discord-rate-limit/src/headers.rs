use std::time::Duration;

use serde_json::Value;

const DEFAULT_RETRY: f64 = 1.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    #[default]
    Route,
    Global,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Headers {
    pub bucket: Option<String>,
    pub remaining: Option<u32>,
    pub reset_after: Option<Duration>,
    pub scope: Scope,
}

impl Headers {
    pub fn read<'a>(header: impl Fn(&str) -> Option<&'a str>) -> Headers {
        Headers {
            bucket: header("x-ratelimit-bucket").map(str::to_owned),
            remaining: header("x-ratelimit-remaining").and_then(|value| value.parse().ok()),
            reset_after: header("x-ratelimit-reset-after").and_then(seconds),
            scope: match header("x-ratelimit-global") {
                Some("true") => Scope::Global,
                _ => Scope::Route,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Throttle {
    pub retry_after: Duration,
    pub scope: Scope,
}

impl Throttle {
    #[must_use]
    pub fn read(headers: &Headers, body: &[u8]) -> Throttle {
        let body: Value = serde_json::from_slice(body).unwrap_or_default();
        let retry_after = body
            .get("retry_after")
            .and_then(Value::as_f64)
            .unwrap_or(DEFAULT_RETRY);
        let global = body.get("global").and_then(Value::as_bool) == Some(true);
        Throttle {
            retry_after: Duration::try_from_secs_f64(retry_after.max(0.0)).unwrap_or_default(),
            scope: match (headers.scope, global) {
                (Scope::Global, _) | (_, true) => Scope::Global,
                (Scope::Route, false) => Scope::Route,
            },
        }
    }
}

fn seconds(text: &str) -> Option<Duration> {
    let seconds: f64 = text.parse().ok()?;
    Duration::try_from_secs_f64(seconds.max(0.0)).ok()
}
