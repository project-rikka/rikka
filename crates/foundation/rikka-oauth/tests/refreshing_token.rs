use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rikka_oauth::{Refresh, RefreshingToken};

#[derive(Clone, Debug)]
struct Tokens {
    generation: usize,
    expires_at: Option<u64>,
    refreshes: Arc<AtomicUsize>,
}

impl Refresh for Tokens {
    type Error = ();

    fn expires_at(&self) -> Option<u64> {
        self.expires_at
    }

    async fn refresh(&self) -> Result<Tokens, ()> {
        self.refreshes.fetch_add(1, Ordering::SeqCst);
        Ok(Tokens {
            generation: self.generation + 1,
            expires_at: Some(now() + 3600),
            refreshes: self.refreshes.clone(),
        })
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after the epoch")
        .as_secs()
}

fn tokens(expires_at: Option<u64>) -> Tokens {
    Tokens {
        generation: 0,
        expires_at,
        refreshes: Arc::new(AtomicUsize::new(0)),
    }
}

#[tokio::test]
async fn refreshes_tokens_without_known_lifetime() {
    let fresh = tokens(Some(now() + 3600));
    let refreshes = fresh.refreshes.clone();
    let refreshing = RefreshingToken::new(fresh);
    assert_eq!(refreshing.current().await.expect("fresh").generation, 0);
    assert_eq!(refreshes.load(Ordering::SeqCst), 0);

    let unknown = tokens(None);
    let refreshes = unknown.refreshes.clone();
    let refreshing = RefreshingToken::new(unknown);
    refreshing.current().await.expect("refreshed");
    refreshing.current().await.expect("still fresh");
    assert_eq!(refreshes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn concurrent_callers_share_one_refresh() {
    let initial = tokens(Some(now() + 60));
    let refreshes = initial.refreshes.clone();
    let notifications = Arc::new(AtomicUsize::new(0));
    let listening = notifications.clone();
    let refreshing = RefreshingToken::new(initial).on_refresh(move |tokens: &Tokens| {
        listening.store(tokens.generation, Ordering::SeqCst);
    });

    let (first, second) = tokio::join!(refreshing.current(), refreshing.current());

    assert_eq!(first.expect("refreshed").generation, 1);
    assert_eq!(second.expect("refreshed").generation, 1);
    assert_eq!(refreshes.load(Ordering::SeqCst), 1);
    assert_eq!(notifications.load(Ordering::SeqCst), 1);
}
