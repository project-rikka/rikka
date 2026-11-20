use std::ops::ControlFlow;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Duration;

use futures_util::StreamExt;
use music_blend::{Blend, Outside, Store, sync};
use music_core::{Artwork, Codec, Error, Fidelity, Isrc, Picture, Resolution};
use rikka_worker::Subscription;

use crate::support::{Rename, SERVICES, Shelf, link, places, settled, track};

const PATIENCE: Duration = Duration::from_secs(5);
const QUIET: Duration = Duration::from_secs(1);

fn forwarded(outside: Subscription<Outside>) -> Receiver<Outside> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        music_core::drain(outside, |change| match sender.send(change) {
            Ok(()) => ControlFlow::Continue(()),
            Err(_) => ControlFlow::Break(()),
        });
    });
    receiver
}

fn jspf_path(root: &Path, name: &str) -> std::path::PathBuf {
    root.join("Playlists").join(format!("{name}.jspf"))
}

#[test]
fn synced_blend_round_trips_as_jspf() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(places(root.path()), SERVICES).unwrap();
    let spotify = Shelf::new("spotify");
    let qobuz = Shelf::new("qobuz");
    qobuz.stock(track("qobuz", "1").named("A"));
    let mut first = track("spotify", "a");
    first.isrc = Isrc::parse("USUM71703861");
    first.fidelity = Some(Fidelity::lossless(Codec::Flac, Resolution::CD));
    first.artwork = Artwork::new(vec![Picture {
        url: "https://art/a".into(),
        width: 300,
    }]);
    let mut blend = Blend::new("Across");
    blend.add(&[first, track("spotify", "b")]);
    let synced = settled(sync(&blend, &[link(&spotify), link(&qobuz)]));
    blend.absorb(&synced);

    store.save(blend.clone());
    drop(store);

    let reopened = Store::open(places(root.path()), SERVICES).unwrap();
    assert_eq!(reopened.blends(), &[Arc::new(blend.clone())]);
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(jspf_path(root.path(), "Across")).unwrap()).unwrap();
    let playlist = &document["playlist"];
    assert_eq!(
        playlist["identifier"],
        format!("urn:uuid:{}", blend.id()).as_str()
    );
    assert_eq!(playlist["track"][0]["title"], "A");
    assert_eq!(playlist["track"][0]["duration"], 180_000);
    assert_eq!(
        playlist["track"][0]["identifier"],
        serde_json::json!(["spotify:a", "qobuz:1"])
    );
}

#[test]
fn file_follows_rename_and_numbers_clashes() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(places(root.path()), SERVICES).unwrap();
    let mut first = Blend::new("Night/Drive");
    let second = Blend::new("Night/Drive");
    store.save(first.clone());
    store.save(second.clone());
    first.rename("Morning");
    store.save(first.clone());
    drop(store);

    assert!(jspf_path(root.path(), "Morning").exists());
    assert!(jspf_path(root.path(), "Night-Drive (2)").exists());
    assert!(!jspf_path(root.path(), "Night-Drive").exists());
    let reopened = Store::open(places(root.path()), SERVICES).unwrap();
    assert_eq!(reopened.blends().len(), 2);
    assert_eq!(reopened.find(first.id()).unwrap().name(), "Morning");
    assert_eq!(reopened.find(second.id()).unwrap().name(), "Night/Drive");
}

#[test]
fn deleted_blend_is_never_rewritten() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(places(root.path()), SERVICES).unwrap();
    let blend = Blend::new("Gone");
    store.save(blend.clone());

    store.delete(blend.id());
    store.save(blend.clone());
    assert!(store.blends().is_empty());
    drop(store);

    assert!(!jspf_path(root.path(), "Gone").exists());
    let reopened = Store::open(places(root.path()), SERVICES).unwrap();
    assert!(reopened.blends().is_empty());
    assert!(reopened.is_deleted(blend.id()));
}

#[test]
fn partial_file_is_skipped_untouched() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(places(root.path()), SERVICES).unwrap();
    let mut blend = Blend::new("Across");
    blend.add(&[track("spotify", "a")]);
    store.save(blend.clone());
    drop(store);
    let saved = jspf_path(root.path(), "Across");
    let broken = jspf_path(root.path(), "broken");
    std::fs::write(&broken, b"not json").unwrap();

    let without_spotify = Store::open(places(root.path()), SERVICES[1..].iter().copied()).unwrap();

    assert!(without_spotify.blends().is_empty());
    assert!(saved.exists());
    assert!(broken.exists());
}

#[test]
fn unwritable_blend_is_reported() {
    let root = tempfile::tempdir().unwrap();
    let blocker = root.path().join("Playlists");
    std::fs::write(&blocker, b"a file, not a folder").unwrap();
    let mut store = Store::open(places(root.path()), SERVICES).unwrap();
    let mut failures = store.failures();

    store.save(Blend::new("Lost"));
    drop(store);

    assert!(matches!(settled(failures.next()), Some(Error::Storage(_))));
}

#[test]
fn outside_edits_reported_own_writes_ignored() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(places(root.path()), SERVICES).unwrap();
    let outside = forwarded(store.outside());
    let mut blend = Blend::new("Shared");
    blend.add(&[track("spotify", "a")]);
    store.save(blend.clone());
    assert_eq!(outside.recv_timeout(QUIET), Err(RecvTimeoutError::Timeout));

    let path = jspf_path(root.path(), "Shared");
    let mut document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    document["playlist"]["title"] = "Edited".into();
    std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    let Ok(Outside::Edited(edited)) = outside.recv_timeout(PATIENCE) else {
        panic!("no outside edit event");
    };
    assert_eq!(edited.id(), blend.id());
    assert_eq!(edited.name(), "Edited");

    std::fs::rename(&path, jspf_path(root.path(), "Renamed by hand")).unwrap();
    assert_eq!(outside.recv_timeout(QUIET), Err(RecvTimeoutError::Timeout));

    std::fs::remove_file(jspf_path(root.path(), "Renamed by hand")).unwrap();
    assert_eq!(
        outside.recv_timeout(PATIENCE),
        Ok(Outside::Gone(blend.id().clone()))
    );
    store.follow(Outside::Gone(blend.id().clone()));
    assert!(store.blends().is_empty());
    assert!(store.is_deleted(blend.id()));
}

#[test]
fn restored_blend_file_returns() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(places(root.path()), SERVICES).unwrap();
    let outside = forwarded(store.outside());
    let blend = Blend::new("Back");
    store.save(blend.clone());
    assert_eq!(outside.recv_timeout(QUIET), Err(RecvTimeoutError::Timeout));
    let saved = std::fs::read(jspf_path(root.path(), "Back")).unwrap();
    store.delete(blend.id());
    assert_eq!(outside.recv_timeout(QUIET), Err(RecvTimeoutError::Timeout));

    std::fs::write(jspf_path(root.path(), "Back"), saved).unwrap();
    let change = outside
        .recv_timeout(PATIENCE)
        .expect("no restored file event");
    store.follow(change);

    assert!(!store.is_deleted(blend.id()));
    assert_eq!(
        store.find(blend.id()).map(|stored| stored.id()),
        Some(blend.id())
    );
    drop(store);
    let reopened = Store::open(places(root.path()), SERVICES).unwrap();
    assert!(!reopened.is_deleted(blend.id()));
    assert_eq!(reopened.blends().len(), 1);
}
