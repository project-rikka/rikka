use std::sync::Arc;

use music_blend::{Blend, sync};
use music_core::Id;

use crate::support::{Rename, Shelf, link, settled, track};

fn mirrored(shelf: &Arc<Shelf>, blend: &Blend) -> Vec<String> {
    let mirror = blend
        .mirrors()
        .iter()
        .find(|mirror| mirror.service.slug == shelf.service().slug)
        .unwrap();
    shelf.content_of(&mirror.playlist)
}

fn synced_once(shelf: &Arc<Shelf>, songs: &[&str]) -> Blend {
    let mut blend = Blend::new("Night");
    let tracks: Vec<_> = songs.iter().map(|id| track("spotify", id)).collect();
    blend.add(&tracks);
    let first = settled(sync(&blend, &[link(shelf)]));
    blend.absorb(&first);
    blend
}

#[test]
fn new_blend_mirrors_to_every_account() {
    let spotify = Shelf::new("spotify");
    let qobuz = Shelf::new("qobuz");
    qobuz.stock(track("qobuz", "q-a").named("A"));
    let mut blend = Blend::new("Night");
    blend.add(&[track("spotify", "a"), track("spotify", "b")]);

    let synced = settled(sync(&blend, &[link(&spotify), link(&qobuz)]));
    blend.absorb(&synced);

    assert!(synced.account_errors().is_empty());
    assert!(blend.is_settled());
    assert_eq!(mirrored(&spotify, &blend), vec!["a", "b"]);
    assert_eq!(mirrored(&qobuz, &blend), vec!["q-a"]);
    assert_eq!(qobuz.names(), vec!["rikka: Night".to_owned()]);
    assert!(
        qobuz
            .description_of(&blend.mirrors()[1].playlist)
            .contains(blend.id().as_str())
    );
    assert_eq!(blend.entries()[0].id_in("qobuz"), Some(&Id::new("q-a")));
    assert_eq!(blend.entries()[1].id_in("qobuz"), None);
}

#[test]
fn mirror_changes_reach_blend_and_mirrors() {
    let spotify = Shelf::new("spotify");
    let qobuz = Shelf::new("qobuz");
    qobuz.stock(track("qobuz", "q-c").named("C"));
    let mut blend = Blend::new("Night");
    blend.add(&[track("spotify", "a"), track("spotify", "b")]);
    let links = [link(&spotify), link(&qobuz)];
    let first = settled(sync(&blend, &links));
    blend.absorb(&first);
    let playlist = blend.mirrors()[0].playlist.clone();

    spotify.arrange_by_hand(&playlist, &["a", "c"], track("spotify", "c").named("C"));
    spotify.rename_by_hand(&playlist, "rikka: Late Night");
    let second = settled(sync(&blend, &links));
    blend.absorb(&second);

    let names: Vec<&str> = blend
        .entries()
        .iter()
        .map(|entry| entry.track().id.as_str())
        .collect();
    assert_eq!(names, vec!["a", "c"]);
    assert_eq!(blend.name(), "Late Night");
    assert!(blend.is_settled());
    assert_eq!(mirrored(&qobuz, &blend), vec!["q-c"]);
    assert_eq!(qobuz.names(), vec!["rikka: Late Night".to_owned()]);
}

#[test]
fn partial_mirror_removes_nothing() {
    let spotify = Shelf::new("spotify");
    let mut blend = synced_once(&spotify, &["a", "b"]);

    spotify.hide_in_pages("b");
    let second = settled(sync(&blend, &[link(&spotify)]));
    blend.absorb(&second);

    assert_eq!(blend.len(), 2);
}

#[test]
fn deleted_mirror_recreated_after_edit() {
    let spotify = Shelf::new("spotify");
    let mut blend = synced_once(&spotify, &["a"]);

    spotify.delete_by_hand(&blend.mirrors()[0].playlist);
    blend.add(&[track("spotify", "b")]);
    let second = settled(sync(&blend, &[link(&spotify)]));
    blend.absorb(&second);

    assert!(!second.is_gone());
    assert_eq!(mirrored(&spotify, &blend), vec!["a", "b"]);
}

#[test]
fn blend_without_mirrors_is_gone() {
    let spotify = Shelf::new("spotify");
    let blend = synced_once(&spotify, &["a"]);

    spotify.delete_by_hand(&blend.mirrors()[0].playlist);
    let second = settled(sync(&blend, &[link(&spotify)]));

    assert!(second.is_gone());
}

#[test]
fn unreachable_account_reported_others_sync() {
    let spotify = Shelf::new("spotify");
    let qobuz = Shelf::new("qobuz");
    qobuz.go_offline();
    let mut blend = Blend::new("Night");
    blend.add(&[track("spotify", "a")]);

    let synced = settled(sync(&blend, &[link(&spotify), link(&qobuz)]));
    blend.absorb(&synced);

    assert_eq!(synced.account_errors().len(), 1);
    assert_eq!(synced.account_errors()[0].service.slug, "qobuz");
    assert!(!blend.is_settled());
    assert_eq!(mirrored(&spotify, &blend), vec!["a"]);
}

#[test]
fn edits_during_sync_stay_pending() {
    let spotify = Shelf::new("spotify");
    let mut blend = Blend::new("Night");
    blend.add(&[track("spotify", "a")]);
    let synced = settled(sync(&blend, &[link(&spotify)]));

    blend.add(&[track("spotify", "b")]);
    blend.absorb(&synced);

    assert_eq!(blend.len(), 2);
    assert!(!blend.is_settled());
}

#[test]
fn local_service_only_matches() {
    let spotify = Shelf::new("spotify");
    let local = Shelf::new("local");
    local.stock(track("local", "l-a").named("A"));
    let mut blend = Blend::new("Night");
    blend.add(&[track("spotify", "a"), track("spotify", "b")]);

    let synced = settled(sync(&blend, &[link(&spotify), link(&local)]));
    blend.absorb(&synced);

    assert!(synced.account_errors().is_empty());
    assert!(blend.is_settled());
    assert!(local.names().is_empty());
    assert_eq!(blend.mirrors().len(), 1);
    assert!(!blend.mirrors()[0].service.is_local());
    assert_eq!(blend.entries()[0].id_in("local"), Some(&Id::new("l-a")));
    assert_eq!(blend.entries()[1].id_in("local"), None);
}

#[test]
fn local_songs_sync_without_local_mirror() {
    let spotify = Shelf::new("spotify");
    let local = Shelf::new("local");
    spotify.stock(track("spotify", "s-a").named("A"));
    let mut blend = Blend::new("Here");
    blend.add(&[track("local", "l-a").named("A")]);

    let alone = settled(sync(&blend, &[link(&local)]));
    blend.absorb(&alone);
    assert!(blend.is_settled());
    assert!(!alone.is_gone());
    assert!(blend.mirrors().is_empty());

    blend.rename("Here and There");
    let both = settled(sync(&blend, &[link(&local), link(&spotify)]));
    blend.absorb(&both);
    assert!(blend.is_settled());
    assert!(local.names().is_empty());
    assert_eq!(mirrored(&spotify, &blend), vec!["s-a"]);
}
