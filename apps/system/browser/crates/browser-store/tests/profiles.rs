use std::sync::Arc;
use std::time::Duration;

use browser_store::{
    ArchiveScope, DataKinds, Decision, DownloadProgress, DownloadState, Error, HistoryQuery,
    NewDownload, Permission, ProfileId, SpaceFields, SpaceIcon, SpaceId, Store, TabRecord,
    TimeRange, Timestamp, Transition, Visit,
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

fn fill(store: &Store, profile: ProfileId, space: SpaceId, url: &str) {
    store.record_visit(
        profile,
        Visit::new(url, Transition::Typed).title("Shared Title"),
    );
    store.set_permission(profile, url, Permission::Camera, Decision::Allow);
    store.set_zoom(profile, "shared.example", 1.5).unwrap();
    store.set_https_only_exception(profile, "shared.example", true);
    let download = store.record_download(
        profile,
        NewDownload {
            url: format!("{url}file"),
            path: "/tmp/file".into(),
            mime: "text/plain".to_owned(),
            total: None,
        },
    );
    store.update_download(
        download,
        DownloadProgress {
            received: 1,
            total: Some(1),
            state: DownloadState::Finished,
        },
    );
    let tab = TabRecord {
        url: url.to_owned(),
        title: "Tab".to_owned(),
        state: Some(Arc::from(vec![1_u8])),
    };
    store.push_closed_tab(space, tab.clone());
    store.archive_tabs(space, vec![tab]);
}

fn history(store: &Store, profile: ProfileId) -> Vec<String> {
    store
        .history(profile, &HistoryQuery::new(10))
        .unwrap()
        .into_iter()
        .map(|entry| entry.url)
        .collect()
}

#[test]
fn profiles_keep_browsing_data_apart() {
    let store = Store::in_memory().unwrap();
    let personal = store.create_profile("Personal");
    let work = store.create_profile("Work");
    let personal_space = space(&store, personal);
    fill(&store, personal, personal_space, "https://shared.example/");
    store.record_visit(
        work,
        Visit::new("https://shared.example/", Transition::Link),
    );
    store.record_visit(work, Visit::new("https://work.example/", Transition::Link));
    store.flush().unwrap();

    assert_eq!(history(&store, personal), ["https://shared.example/"]);
    assert_eq!(history(&store, work).len(), 2);
    let personal_top = store.top_sites(personal, 10).unwrap();
    let work_top = store.top_sites(work, 10).unwrap();
    assert_eq!(personal_top.len(), 1);
    assert_eq!(
        (personal_top[0].visit_count, personal_top[0].typed_count),
        (1, 1)
    );
    assert_eq!(work_top.len(), 2);
    assert_ne!(
        personal_top[0].id,
        work_top
            .iter()
            .find(|p| p.url == personal_top[0].url)
            .unwrap()
            .id
    );
    assert!(store.suggest(work, "Shared Title", 5).unwrap().is_empty());
    assert_eq!(store.suggest(personal, "Shared Title", 5).unwrap().len(), 1);
    assert!(store.suggest(personal, "work", 5).unwrap().is_empty());

    assert_eq!(
        store
            .permission(personal, "https://shared.example/", Permission::Camera)
            .unwrap(),
        Some(Decision::Allow)
    );
    assert_eq!(
        store
            .permission(work, "https://shared.example/", Permission::Camera)
            .unwrap(),
        None
    );
    assert_eq!(store.zoom(work, "shared.example").unwrap(), None);
    assert!(store.https_only_exceptions(work).unwrap().is_empty());
    assert!(store.sites_with_settings(work).unwrap().is_empty());
    assert_eq!(store.sites_with_settings(personal).unwrap().len(), 1);
    assert!(store.downloads(work, 10).unwrap().is_empty());
    assert_eq!(store.downloads(personal, 10).unwrap().len(), 1);

    store.clear_history(work);
    store.flush().unwrap();
    assert!(history(&store, work).is_empty());
    assert_eq!(history(&store, personal), ["https://shared.example/"]);
}

#[test]
fn clearing_browsing_data_touches_one_profile() {
    let store = Store::in_memory().unwrap();
    let personal = store.create_profile("Personal");
    let work = store.create_profile("Work");
    let personal_space = space(&store, personal);
    let work_space = space(&store, work);
    fill(&store, personal, personal_space, "https://shared.example/");
    fill(&store, work, work_space, "https://shared.example/");
    store.flush().unwrap();

    store.clear_browsing_data(personal, DataKinds::all(), TimeRange::all());
    store.flush().unwrap();

    assert!(history(&store, personal).is_empty());
    assert!(store.downloads(personal, 10).unwrap().is_empty());
    assert!(store.sites_with_settings(personal).unwrap().is_empty());
    assert!(
        store
            .search_archive(ArchiveScope::Profile(personal), "", 10)
            .unwrap()
            .is_empty()
    );
    assert_eq!(history(&store, work).len(), 1);
    assert_eq!(store.downloads(work, 10).unwrap().len(), 1);
    assert_eq!(store.sites_with_settings(work).unwrap().len(), 1);
    assert_eq!(
        store
            .search_archive(ArchiveScope::Profile(work), "", 10)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        store
            .search_archive(ArchiveScope::Space(work_space), "", 10)
            .unwrap()
            .len(),
        1
    );
    let closed = store.closed_tabs().unwrap();
    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0].space, work_space);
}

#[test]
fn profile_removal_deletes_data() {
    let store = Store::in_memory().unwrap();
    let personal = store.create_profile("Personal");
    let work = store.create_profile("Work");
    let work_space = space(&store, work);
    let personal_space = space(&store, personal);
    fill(&store, work, work_space, "https://shared.example/");
    fill(&store, personal, personal_space, "https://shared.example/");
    store.remove_space(work_space);
    store.remove_profile(work).unwrap();

    assert!(history(&store, work).is_empty());
    assert!(store.top_sites(work, 10).unwrap().is_empty());
    assert!(store.sites_with_settings(work).unwrap().is_empty());
    assert!(store.downloads(work, 10).unwrap().is_empty());
    assert_eq!(history(&store, personal).len(), 1);
    assert_eq!(store.sites_with_settings(personal).unwrap().len(), 1);
    assert_eq!(store.downloads(personal, 10).unwrap().len(), 1);
    assert_eq!(store.suggest(personal, "shared", 5).unwrap().len(), 1);
}

fn tab(url: &str) -> TabRecord {
    TabRecord {
        url: url.to_owned(),
        title: url.to_owned(),
        state: None,
    }
}

#[test]
fn clearing_respects_kinds_and_range() {
    let store = Store::in_memory().unwrap();
    let (profile, space) = support::space(&store, "Home");
    let long_ago = Timestamp::now().saturating_sub(Duration::from_secs(7 * 24 * 3600));
    store.record_visit(
        profile,
        Visit::new("https://old.example/", Transition::Link).at(long_ago),
    );
    store.flush().unwrap();

    let since = Timestamp::now();
    store.record_visit(
        profile,
        Visit::new("https://recent.example/", Transition::Link),
    );
    let download = store.record_download(
        profile,
        NewDownload {
            url: "https://files.example/a".to_owned(),
            path: "/tmp/a".into(),
            mime: "text/plain".to_owned(),
            total: None,
        },
    );
    store.update_download(
        download,
        browser_store::DownloadProgress {
            received: 1,
            total: Some(1),
            state: browser_store::DownloadState::Finished,
        },
    );
    store.push_closed_tab(space, tab("https://closed.example/"));
    store.archive_tabs(space, vec![tab("https://archived.example/")]);
    store.set_permission(
        profile,
        "https://recent.example",
        Permission::Popups,
        Decision::Allow,
    );
    store.flush().unwrap();

    store.clear_browsing_data(
        profile,
        DataKinds {
            archive: false,
            ..DataKinds::all()
        },
        TimeRange::since(since),
    );
    store.flush().unwrap();

    let history: Vec<_> = store
        .history(profile, &HistoryQuery::new(10))
        .unwrap()
        .into_iter()
        .map(|entry| entry.url)
        .collect();
    assert_eq!(history, ["https://old.example/"]);
    assert!(store.downloads(profile, 10).unwrap().is_empty());
    assert!(store.closed_tabs().unwrap().is_empty());
    assert_eq!(
        store
            .search_archive(ArchiveScope::Everywhere, "", 10)
            .unwrap()
            .len(),
        1
    );
    assert!(store.sites_with_settings(profile).unwrap().is_empty());

    store.clear_browsing_data(profile, DataKinds::all(), TimeRange::all());
    store.flush().unwrap();
    assert!(
        store
            .history(profile, &HistoryQuery::new(10))
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .search_archive(ArchiveScope::Everywhere, "", 10)
            .unwrap()
            .is_empty()
    );
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
