use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::headers::{Headers, Scope};

#[derive(Clone, Copy, Debug)]
struct Window {
    remaining: u32,
    resets: Instant,
}

#[derive(Debug, Default)]
pub struct Limiter {
    buckets: HashMap<String, String>,
    windows: HashMap<String, Window>,
    global_until: Option<Instant>,
}

impl Limiter {
    pub fn take(&mut self, route: &Route, now: Instant) -> Option<Duration> {
        let global = self
            .global_until
            .filter(|until| *until > now)
            .map(|until| until - now);
        let key = self.window_key(route);
        let local = match self.windows.get_mut(&key) {
            Some(window) if window.resets <= now => {
                self.windows.remove(&key);
                None
            }
            Some(window) if window.remaining == 0 => Some(window.resets - now),
            Some(window) => {
                window.remaining -= 1;
                None
            }
            None => None,
        };
        match (global, local) {
            (Some(global), Some(local)) => Some(global.max(local)),
            (wait, None) | (None, wait) => wait,
        }
    }

    pub fn record(&mut self, route: &Route, headers: &Headers, now: Instant) {
        if let Some(bucket) = &headers.bucket {
            self.buckets.insert(route.template.clone(), bucket.clone());
        }
        let (Some(remaining), Some(reset_after)) = (headers.remaining, headers.reset_after) else {
            return;
        };
        let window = Window {
            remaining,
            resets: now + reset_after,
        };
        self.windows.insert(self.window_key(route), window);
    }

    pub fn throttle(&mut self, route: &Route, retry_after: Duration, scope: Scope, now: Instant) {
        let until = now + retry_after;
        match scope {
            Scope::Global => {
                self.global_until = Some(self.global_until.map_or(until, |known| known.max(until)));
            }
            Scope::Route => {
                let window = Window {
                    remaining: 0,
                    resets: until,
                };
                self.windows.insert(self.window_key(route), window);
            }
        }
    }

    fn window_key(&self, route: &Route) -> String {
        let bucket = self
            .buckets
            .get(&route.template)
            .map_or(route.template.as_str(), String::as_str);
        format!("{bucket}:{}", route.major)
    }
}

const MAJORS: [&str; 3] = ["channels", "guilds", "webhooks"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    template: String,
    major: String,
}

impl Route {
    #[must_use]
    pub fn new(method: &str, path: &str) -> Route {
        let path = path.split('?').next().unwrap_or_default();
        let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
        let mut major = String::new();
        let mut template = Vec::with_capacity(segments.len());
        for (at, segment) in segments.iter().enumerate() {
            let previous = at.checked_sub(1).map(|before| segments[before]);
            if previous == Some("reactions") {
                template.push("*");
                break;
            }
            let numeric = !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_digit());
            if numeric && at == 1 && previous.is_some_and(|name| MAJORS.contains(&name)) {
                major = (*segment).to_owned();
                template.push(":major");
            } else if numeric {
                template.push(":id");
            } else {
                template.push(segment);
            }
        }
        Route {
            template: format!("{method} /{}", template.join("/")),
            major,
        }
    }

    #[must_use]
    pub fn template(&self) -> &str {
        &self.template
    }

    #[must_use]
    pub fn major(&self) -> &str {
        &self.major
    }
}
