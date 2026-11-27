use browser_store::{BookmarkLink, BookmarkRoots, BookmarkSync, LinkedItem, PinnedPlace, Store};

use crate::support;

fn link(bookmark: &str, item: LinkedItem, parent: &str, position: u32) -> BookmarkLink {
    BookmarkLink {
        bookmark: bookmark.to_owned(),
        item,
        parent: parent.to_owned(),
        position,
        title: format!("Title {bookmark}"),
        url: (!matches!(item, LinkedItem::Space(_) | LinkedItem::Folder(_)))
            .then(|| format!("https://{bookmark}.example/")),
    }
}

#[test]
fn bookmark_sync_round_trips_and_replaces_per_profile() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("browser.sqlite");
    let store = Store::open(&path).unwrap();
    let (profile, space) = support::space(&store, "Home");
    let other = store.create_profile("Other");
    let favorite = store.add_favorite(profile, "https://a.example/", "A");
    let pinned_folder = store.create_pinned_folder(PinnedPlace::root(space), "Folder");
    let pin = store.pin_tab(
        PinnedPlace::folder(pinned_folder),
        "https://b.example/",
        None,
    );
    let roots = BookmarkRoots {
        rikka: "100".to_owned(),
        favorites: "101".to_owned(),
        spaces: "102".to_owned(),
    };
    let first = BookmarkSync {
        roots: roots.clone(),
        links: vec![
            link("103", LinkedItem::Favorite(favorite), "101", 0),
            link("104", LinkedItem::Space(space), "102", 0),
            link("105", LinkedItem::Folder(pinned_folder), "104", 0),
            link("106", LinkedItem::Pin(pin), "105", 0),
        ],
    };
    let replaced = BookmarkSync {
        roots,
        links: vec![link("107", LinkedItem::Space(space), "102", 0)],
    };
    assert_eq!(store.bookmark_sync(profile).unwrap(), None);
    store.save_bookmark_sync(profile, first.clone());
    store.save_bookmark_sync(other, replaced.clone());
    store.flush().unwrap();
    drop(store);

    let store = Store::open(&path).unwrap();
    let mut loaded = store.bookmark_sync(profile).unwrap().expect("saved sync");
    loaded
        .links
        .sort_by(|left, right| left.bookmark.cmp(&right.bookmark));
    assert_eq!(loaded, first);
    store.save_bookmark_sync(profile, replaced.clone());
    store.flush().unwrap();
    assert_eq!(
        store.bookmark_sync(profile).unwrap(),
        Some(replaced.clone())
    );
    assert_eq!(store.bookmark_sync(other).unwrap(), Some(replaced));
}
