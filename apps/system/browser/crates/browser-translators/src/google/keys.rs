use std::fmt;
use std::future::Future;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

const KEY_LIFETIME: Duration = Duration::from_secs(20 * 60);
const RETRY_AFTER_FAILURE: Duration = Duration::from_secs(60);
const KEY_LENGTH: usize = 39;

#[derive(Default)]
pub(crate) struct KeyCache {
    state: Mutex<KeyState>,
}

#[derive(Default)]
struct KeyState {
    key: Option<(String, Instant)>,
    failed: Option<Instant>,
}

impl fmt::Debug for KeyCache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("KeyCache");
        match self.state.try_lock() {
            Ok(state) => debug
                .field("known", &state.key.is_some())
                .field("failed", &state.failed),
            Err(_) => debug.field("busy", &true),
        };
        debug.finish()
    }
}

impl KeyCache {
    pub(crate) async fn get<F, Fut>(&self, scrape: F) -> Option<String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<String>>,
    {
        let mut state = self.state.lock().await;
        if let Some((key, fetched)) = &state.key
            && fetched.elapsed() < KEY_LIFETIME
        {
            return Some(key.clone());
        }
        if state
            .failed
            .is_some_and(|failed| failed.elapsed() < RETRY_AFTER_FAILURE)
        {
            return None;
        }
        let scraped = scrape().await;
        state.key = scraped.clone().map(|key| (key, Instant::now()));
        state.failed = scraped.is_none().then(Instant::now);
        scraped
    }

    pub(crate) async fn invalidate(&self, rejected: &str) {
        let mut state = self.state.lock().await;
        if state.key.as_ref().is_some_and(|(key, _)| key == rejected) {
            state.key = None;
        }
    }
}

pub(crate) fn find_key<'a>(page: &'a str, name: &str) -> Option<&'a str> {
    let lower = page.to_ascii_lowercase();
    let name = name.to_ascii_lowercase();
    let bytes = page.as_bytes();
    let mut from = 0;
    while let Some(offset) = lower[from..].find(&name) {
        let start = from + offset;
        from = start + name.len();
        if start == 0 || !is_quote(bytes[start - 1]) {
            continue;
        }
        let Some(&close) = bytes.get(from) else {
            continue;
        };
        if !is_quote(close) {
            continue;
        }
        let mut at = skip_spaces(bytes, from + 1);
        if bytes.get(at) != Some(&b':') {
            continue;
        }
        at = skip_spaces(bytes, at + 1);
        if !bytes.get(at).copied().is_some_and(is_quote) {
            continue;
        }
        let key_start = at + 1;
        let key_end = key_start + KEY_LENGTH;
        if bytes.len() > key_end
            && bytes[key_start..key_end]
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
            && is_quote(bytes[key_end])
        {
            return Some(&page[key_start..key_end]);
        }
    }
    None
}

fn is_quote(byte: u8) -> bool {
    byte == b'"' || byte == b'\''
}

fn skip_spaces(bytes: &[u8], mut at: usize) -> usize {
    while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
        at += 1;
    }
    at
}
