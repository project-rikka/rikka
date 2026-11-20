use music_blend::{BlendId, Store, discover};
use music_core::Id;

use crate::support::{Rename, SERVICES, Shelf, link, places, settled, track};

fn order() -> Vec<String> {
    vec!["qobuz".into(), "spotify".into()]
}

fn marked(id: &BlendId) -> String {
    format!("rikka:{id}")
}

#[test]
fn mirrors_rebuild_blend_in_order() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(places(root.path()), SERVICES).unwrap();
    let spotify = Shelf::new("spotify");
    let qobuz = Shelf::new("qobuz");
    let id = BlendId::generate();
    spotify.create_unmarked_playlist(
        "rikka: Night",
        &marked(&id),
        vec![
            track("spotify", "a").named("A"),
            track("spotify", "only").named("Only Here"),
            track("spotify", "c").named("C"),
        ],
    );
    qobuz.create_unmarked_playlist(
        "rikka: Night",
        &marked(&id),
        vec![
            track("qobuz", "q-a").named("A"),
            track("qobuz", "q-c").named("C"),
            track("qobuz", "q-z").named("Z"),
        ],
    );

    let discovered = settled(discover(
        store.snapshot(),
        vec![link(&spotify), link(&qobuz)],
        order(),
    ));

    assert_eq!(discovered.blends().len(), 1);
    let blend = &discovered.blends()[0];
    assert_eq!(blend.id(), &id);
    assert_eq!(blend.name(), "Night");
    let uris: Vec<String> = blend
        .entries()
        .iter()
        .map(|entry| entry.track().uri())
        .collect();
    assert_eq!(
        uris,
        vec!["qobuz:q-a", "spotify:only", "qobuz:q-c", "qobuz:q-z"]
    );
    assert_eq!(blend.entries()[0].id_in("spotify"), Some(&Id::new("a")));
    assert_eq!(blend.mirrors().len(), 2);
}

#[test]
fn deleted_blend_mirror_is_removed() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(places(root.path()), SERVICES).unwrap();
    let spotify = Shelf::new("spotify");
    let id = BlendId::generate();
    spotify.create_unmarked_playlist("rikka: Gone", &marked(&id), Vec::new());
    store.delete(&id);

    let discovered = settled(discover(store.snapshot(), vec![link(&spotify)], order()));

    assert!(discovered.blends().is_empty());
    assert!(spotify.names().is_empty());
}

#[test]
fn local_service_is_not_surveyed() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(places(root.path()), SERVICES).unwrap();
    let local = Shelf::new("local");
    let deleted = BlendId::generate();
    let unknown = BlendId::generate();
    local.create_unmarked_playlist("rikka: Gone", &marked(&deleted), Vec::new());
    local.create_unmarked_playlist(
        "rikka: Night",
        &marked(&unknown),
        vec![track("local", "l-a").named("A")],
    );
    store.delete(&deleted);

    let discovered = settled(discover(store.snapshot(), vec![link(&local)], order()));

    assert!(discovered.blends().is_empty());
    assert!(discovered.adopted().is_empty());
    assert!(discovered.account_errors().is_empty());
    assert_eq!(local.names().len(), 2);
}
