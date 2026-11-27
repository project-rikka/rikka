use std::time::Duration;

use browser_store::{
    ArchiveScope, BookmarkLink, BookmarkRoots, BookmarkSync, DataKinds, Error, LinkedItem,
    ProfileId, SpaceFields, SpaceIcon, SpaceId, Store, TabRecord, TimeRange, Timestamp,
};

use serde::{Deserialize, Serialize};

use crate::support;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Settings {
    tracking: String,
    languages: Vec<String>,
}

fn space(store: &Store, profile: ProfileId) -> SpaceId {
    store.create_space(SpaceFields {
        name: "Space".to_owned(),
        icon: SpaceIcon::Symbol("home".to_owned()),
        profile,
    })
}

fn tab(url: &str) -> TabRecord {
    TabRecord {
        url: url.to_owned(),
        title: url.to_owned(),
    }
}

fn fill(store: &Store, space: SpaceId, url: &str) {
    store.push_closed_tab(space, tab(url));
    store.archive_tabs(space, vec![tab(url)]);
}

fn archived(store: &Store, scope: ArchiveScope) -> usize {
    store.search_archive(scope, "", 10).unwrap().len()
}

#[test]
fn clearing_browsing_data_touches_one_profile() {
    let store = Store::in_memory().unwrap();
    let personal = store.create_profile("Personal");
    let work = store.create_profile("Work");
    let personal_space = space(&store, personal);
    let work_space = space(&store, work);
    fill(&store, personal_space, "https://shared.example/");
    fill(&store, work_space, "https://shared.example/");
    store.flush().unwrap();

    store.clear_browsing_data(personal, DataKinds::all(), TimeRange::all());
    store.flush().unwrap();

    assert_eq!(archived(&store, ArchiveScope::Profile(personal)), 0);
    assert_eq!(archived(&store, ArchiveScope::Profile(work)), 1);
    assert_eq!(archived(&store, ArchiveScope::Space(work_space)), 1);
    let closed = store.closed_tabs().unwrap();
    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0].space, work_space);
}

#[test]
fn clearing_respects_kinds_and_range() {
    let store = Store::in_memory().unwrap();
    let (profile, space) = support::space(&store, "Home");
    let hour_ago = Timestamp::now().saturating_sub(Duration::from_secs(3600));
    store.push_closed_tab(space, tab("https://closed.example/"));
    store.archive_tabs(space, vec![tab("https://archived.example/")]);
    store.flush().unwrap();

    store.clear_browsing_data(profile, DataKinds::all(), TimeRange::before(hour_ago));
    store.flush().unwrap();
    assert_eq!(store.closed_tabs().unwrap().len(), 1);
    assert_eq!(archived(&store, ArchiveScope::Everywhere), 1);

    store.clear_browsing_data(
        profile,
        DataKinds {
            archive: false,
            ..DataKinds::all()
        },
        TimeRange::since(hour_ago),
    );
    store.flush().unwrap();
    assert!(store.closed_tabs().unwrap().is_empty());
    assert_eq!(archived(&store, ArchiveScope::Everywhere), 1);

    store.clear_browsing_data(profile, DataKinds::all(), TimeRange::since(hour_ago));
    store.flush().unwrap();
    assert_eq!(archived(&store, ArchiveScope::Everywhere), 0);
}

#[test]
fn profile_removal_deletes_its_data() {
    let store = Store::in_memory().unwrap();
    let personal = store.create_profile("Personal");
    let work = store.create_profile("Work");
    let work_space = space(&store, work);
    let personal_space = space(&store, personal);
    let favorite = store.add_favorite(work, "https://work.example/", "Work");
    store.set_profile_settings(work, &"strict").unwrap();
    store.save_bookmark_sync(
        work,
        BookmarkSync {
            roots: BookmarkRoots {
                rikka: "10".to_owned(),
                favorites: "11".to_owned(),
                spaces: "12".to_owned(),
            },
            links: vec![BookmarkLink {
                bookmark: "13".to_owned(),
                item: LinkedItem::Favorite(favorite),
                parent: "11".to_owned(),
                position: 0,
                title: "Work".to_owned(),
                url: Some("https://work.example/".to_owned()),
            }],
        },
    );
    fill(&store, personal_space, "https://personal.example/");
    store.remove_space(work_space);
    store.remove_profile(work).unwrap();

    assert!(store.favorites(work).unwrap().is_empty());
    assert_eq!(store.profile_settings::<String>(work).unwrap(), None);
    assert_eq!(store.bookmark_sync(work).unwrap(), None);
    assert_eq!(archived(&store, ArchiveScope::Profile(personal)), 1);
}

#[test]
fn profile_in_use_cannot_be_removed() {
    let store = Store::in_memory().unwrap();
    let (profile, space) = support::space(&store, "Home");
    store.create_profile("Spare");

    assert!(matches!(store.remove_profile(profile), Err(Error::ProfileInUse(id)) if id == profile));
    store.remove_space(space);
    store.remove_profile(profile).unwrap();
}

#[test]
fn profile_settings_persist_per_profile() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("browser.sqlite");
    let store = Store::open(&path).unwrap();
    let personal = store.create_profile("Personal");
    let work = store.create_profile("Work");
    let first = Settings {
        tracking: "standard".to_owned(),
        languages: vec!["ja".to_owned()],
    };
    let last = Settings {
        tracking: "strict".to_owned(),
        languages: vec!["ja".to_owned(), "en".to_owned()],
    };
    store.set_profile_settings(personal, &first).unwrap();
    store.set_profile_settings(personal, &last).unwrap();
    store.set_profile_settings(work, &first).unwrap();
    store.flush().unwrap();
    drop(store);

    let store = Store::open(&path).unwrap();
    assert_eq!(store.profile_settings(personal).unwrap(), Some(last));
    assert_eq!(store.profile_settings(work).unwrap(), Some(first));
    store.remove_profile(work).unwrap();
    assert_eq!(store.profile_settings::<Settings>(work).unwrap(), None);
    assert!(matches!(
        store.profile_settings::<Vec<u32>>(personal),
        Err(Error::MalformedSettings { .. })
    ));
}
