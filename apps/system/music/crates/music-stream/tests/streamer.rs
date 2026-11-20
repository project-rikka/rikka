use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use music_core::{
    AudioCache, Error, Id, Playback, PlaybackEvent, ProviderCache, Quality, Result, async_trait,
};
use music_stream::{Origin, Spool, Streamer};

use crate::support::{PATIENCE, deck, forwarded, wait_for, wav};

fn wait_until_ready(events: &Receiver<PlaybackEvent>) {
    let mut seen = Vec::new();
    let ready = wait_for(events, &mut seen, |event| {
        matches!(
            event,
            PlaybackEvent::Playing { .. }
                | PlaybackEvent::Failed(_)
                | PlaybackEvent::Unavailable { .. }
        )
    });
    match ready {
        Some(PlaybackEvent::Unavailable { .. }) => {
            panic!("the track could not be opened: {seen:?}")
        }
        Some(_) => {}
        None => panic!("the player never readied the track: {seen:?}"),
    }
}

fn cached_within(cache: &ProviderCache, key: &str) -> bool {
    let deadline = std::time::Instant::now() + PATIENCE;
    while std::time::Instant::now() < deadline {
        if cache.find(key).is_some() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[derive(Default)]
struct Counted {
    fetches: AtomicUsize,
}

#[async_trait]
impl Origin for Counted {
    async fn fetch(&self, _track: &Id, _quality: Quality, spool: &mut Spool) -> Result<()> {
        self.fetches.fetch_add(1, Ordering::SeqCst);
        let whole = wav(48_000, 4_800);
        spool.begin(Some(whole.len() as u64));
        spool.write(&whole)
    }
}

struct Stalled;

#[async_trait]
impl Origin for Stalled {
    async fn fetch(&self, _track: &Id, _quality: Quality, spool: &mut Spool) -> Result<()> {
        let whole = wav(48_000, 48_000 * 60);
        spool.begin(Some(whole.len() as u64));
        spool.write(&whole[..256 * 1024])?;
        std::future::pending().await
    }
}

#[test]
fn finished_download_is_cached() {
    let root = tempfile::tempdir().unwrap();
    let cache = AudioCache::open(root.path(), 1024 * 1024).for_provider("fake");
    let origin = Arc::new(Counted::default());
    let streamer = Streamer::start(origin.clone(), cache.clone(), deck()).unwrap();
    let events = forwarded(streamer.events());
    let track = Id::new("t");

    streamer.load(&track, Duration::ZERO, true);
    wait_until_ready(&events);
    assert!(cached_within(&cache, "t.highest"));

    streamer.load(&track, Duration::ZERO, true);
    wait_until_ready(&events);
    assert_eq!(origin.fetches.load(Ordering::SeqCst), 1);
}

#[test]
fn playback_starts_during_download() {
    let root = tempfile::tempdir().unwrap();
    let cache = AudioCache::open(root.path(), 0).for_provider("fake");
    let streamer = Streamer::start(Arc::new(Stalled), cache, deck()).unwrap();
    let events = forwarded(streamer.events());

    streamer.load(&Id::new("t"), Duration::ZERO, true);

    wait_until_ready(&events);
}

#[test]
fn paused_load_stays_silent() {
    let root = tempfile::tempdir().unwrap();
    let cache = AudioCache::open(root.path(), 1024 * 1024).for_provider("fake");
    let streamer = Streamer::start(Arc::new(Counted::default()), cache, deck()).unwrap();
    let events = forwarded(streamer.events());
    let track = Id::new("t");

    streamer.load(&track, Duration::from_millis(20), false);

    let mut seen = Vec::new();
    let paused = wait_for(&events, &mut seen, |event| {
        matches!(
            event,
            PlaybackEvent::Paused { .. } | PlaybackEvent::Unavailable { .. }
        )
    });
    assert_eq!(
        paused,
        Some(PlaybackEvent::Paused {
            track,
            at: Duration::from_millis(20)
        }),
        "{seen:?}"
    );
    assert!(
        seen.iter().any(|event| matches!(event, PlaybackEvent::Length { length, .. } if *length == Duration::from_millis(100))),
        "{seen:?}"
    );
    assert!(
        !seen
            .iter()
            .any(|event| matches!(event, PlaybackEvent::Playing { .. }))
    );
}

struct Refusing;

#[async_trait]
impl Origin for Refusing {
    async fn fetch(&self, track: &Id, _quality: Quality, _spool: &mut Spool) -> Result<()> {
        Err(Error::Refused(format!("no key for {track}")))
    }
}

#[test]
fn refused_fetch_is_reported_as_refused() {
    let root = tempfile::tempdir().unwrap();
    let cache = AudioCache::open(root.path(), 0).for_provider("fake");
    let streamer = Streamer::start(Arc::new(Refusing), cache, deck()).unwrap();
    let events = forwarded(streamer.events());
    let track = Id::new("t");

    streamer.load(&track, Duration::ZERO, true);

    let mut seen = Vec::new();
    let ended = wait_for(&events, &mut seen, |event| {
        matches!(
            event,
            PlaybackEvent::Refused { .. } | PlaybackEvent::Unavailable { .. }
        )
    });
    assert_eq!(ended, Some(PlaybackEvent::Refused { track }), "{seen:?}");
}
