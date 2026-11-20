use std::time::Duration;

use browser_store::{
    Error, HistoryQuery, Picture, ProfileId, Store, TimeRange, Timestamp, Transition, Visit,
};

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

fn ago(duration: Duration) -> Timestamp {
    Timestamp::now().saturating_sub(duration)
}

fn visit(
    store: &Store,
    profile: ProfileId,
    url: &str,
    title: &str,
    transition: Transition,
    at: Timestamp,
) {
    store.record_visit(profile, Visit::new(url, transition).title(title).at(at));
}

#[test]
fn history_pages_newest_first() {
    let store = Store::in_memory().unwrap();
    let profile = store.create_profile("Default");
    for hour in 0..10_u64 {
        visit(
            &store,
            profile,
            &format!("https://example.com/{hour}"),
            "Page",
            Transition::Link,
            ago(Duration::from_secs(hour * 3600)),
        );
    }
    store.flush().unwrap();

    let first = store.history(profile, &HistoryQuery::new(4)).unwrap();
    assert_eq!(first[0].url, "https://example.com/0");
    let second = store
        .history(
            profile,
            &HistoryQuery::new(4).after(first.last().unwrap().cursor()),
        )
        .unwrap();
    assert_eq!(second[0].url, "https://example.com/4");

    let window = TimeRange::new(
        ago(Duration::from_secs(5 * 3600 + 60)),
        ago(Duration::from_secs(2 * 3600 + 60)),
    );
    let ranged: Vec<_> = store
        .history(profile, &HistoryQuery::new(10).range(window))
        .unwrap()
        .into_iter()
        .map(|entry| entry.url)
        .collect();
    assert_eq!(
        ranged,
        [
            "https://example.com/3",
            "https://example.com/4",
            "https://example.com/5"
        ]
    );
}

#[test]
fn deleting_visits_updates_places_and_removes_orphans() {
    let store = Store::in_memory().unwrap();
    let profile = store.create_profile("Default");
    visit(
        &store,
        profile,
        "https://kept.example/",
        "Kept",
        Transition::Typed,
        ago(DAY * 3),
    );
    visit(
        &store,
        profile,
        "https://kept.example/",
        "Kept",
        Transition::Link,
        ago(Duration::from_secs(60)),
    );
    visit(
        &store,
        profile,
        "https://gone.example/",
        "Gone",
        Transition::Link,
        ago(Duration::from_secs(60)),
    );
    visit(
        &store,
        profile,
        "https://url.example/",
        "Url",
        Transition::Link,
        ago(DAY * 3),
    );
    store.delete_range(profile, TimeRange::since(ago(DAY)));
    store.delete_url(profile, "https://url.example/");
    store.flush().unwrap();

    let places = store.top_sites(profile, 10).unwrap();
    assert_eq!(places.len(), 1);
    assert_eq!(places[0].url, "https://kept.example/");
    assert_eq!(places[0].visit_count, 1);
    assert_eq!(places[0].typed_count, 1);
    assert!(store.suggest(profile, "gone", 5).unwrap().is_empty());
    assert!(
        store
            .history(profile, &HistoryQuery::new(10).text("Gone"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn pruning_and_clearing_remove_old_and_all_history() {
    let store = Store::in_memory().unwrap();
    let profile = store.create_profile("Default");
    visit(
        &store,
        profile,
        "https://old.example/",
        "Old",
        Transition::Link,
        ago(DAY * 120),
    );
    visit(
        &store,
        profile,
        "https://new.example/",
        "New",
        Transition::Link,
        ago(DAY),
    );
    store.prune_history(profile, ago(DAY * 90));
    store.flush().unwrap();
    let left: Vec<_> = store
        .history(profile, &HistoryQuery::new(10))
        .unwrap()
        .into_iter()
        .map(|entry| entry.url)
        .collect();
    assert_eq!(left, ["https://new.example/"]);

    store.clear_history(profile);
    store.flush().unwrap();
    assert!(
        store
            .history(profile, &HistoryQuery::new(10))
            .unwrap()
            .is_empty()
    );
    assert!(store.top_sites(profile, 10).unwrap().is_empty());
}

#[test]
fn visited_queries_scoped_to_profile() {
    let store = Store::in_memory().unwrap();
    let profile = store.create_profile("Default");
    let other = store.create_profile("Work");
    visit(
        &store,
        profile,
        "https://a_b.example:8443/page",
        "Page",
        Transition::Link,
        ago(DAY * 2),
    );
    store.flush().unwrap();
    let urls = [
        "https://a_b.example:8443/page".to_owned(),
        "https://a_b.example:8443/other".to_owned(),
    ];
    assert_eq!(store.visited(profile, &urls).unwrap(), [true, false]);
    assert_eq!(store.visited(other, &urls).unwrap(), [false, false]);
    assert!(
        store
            .host_visited(profile, "A_B.example", TimeRange::since(ago(DAY * 3)))
            .unwrap()
    );
    assert!(
        !store
            .host_visited(profile, "a_b.example", TimeRange::since(ago(DAY)))
            .unwrap()
    );
    assert!(
        !store
            .host_visited(profile, "aXb.example", TimeRange::all())
            .unwrap()
    );
    assert!(
        !store
            .host_visited(other, "a_b.example", TimeRange::all())
            .unwrap()
    );
}

#[test]
fn host_visits_match_the_visited_host_only() {
    let store = Store::in_memory().unwrap();
    let profile = store.create_profile("Default");
    for url in [
        "https://tracker.example/r?to=https://lure.example/",
        "https://lure.example.evil.test/",
        "https://www.shop.example/cart",
        "http://user@[::1]:8080/",
    ] {
        visit(&store, profile, url, "Page", Transition::Link, ago(DAY));
    }
    store.flush().unwrap();
    let cases = [
        ("lure.example", false),
        ("tracker.example", true),
        ("lure.example.evil.test", true),
        ("shop.example", false),
        ("www.shop.example.", true),
        ("::1", true),
    ];
    for (host, visited) in cases {
        assert_eq!(
            store.host_visited(profile, host, TimeRange::all()).unwrap(),
            visited,
            "{host}"
        );
    }
}

fn solid(side: u32, rgba: [u8; 4]) -> Picture {
    Picture::new(side, side, rgba.repeat((side * side) as usize)).unwrap()
}

#[test]
fn clearing_history_drops_orphan_icons() {
    let store = Store::in_memory().unwrap();
    let profile = store.create_profile("Default");
    store.record_visit(
        profile,
        Visit::new("https://seen.example/", Transition::Link),
    );
    store.set_favicon("https://seen.example/", solid(8, [1, 2, 3, 255]));
    store.flush().unwrap();
    assert!(
        store
            .favicon_for_url("https://seen.example/")
            .unwrap()
            .is_some()
    );

    store.clear_history(profile);
    store.flush().unwrap();
    assert!(
        store
            .history(profile, &HistoryQuery::new(1))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store.favicon_for_url("https://seen.example/").unwrap(),
        None
    );
}

#[test]
fn rejects_malformed_favicons() {
    assert!(matches!(
        Picture::new(4, 4, vec![0; 10]),
        Err(Error::InvalidPicture {
            expected: 64,
            actual: 10,
            ..
        })
    ));
    assert!(Picture::new(0, 0, Vec::new()).is_err());
    assert!(matches!(
        Picture::new(65, 8, vec![0; 65 * 8 * 4]),
        Err(Error::OversizedPicture {
            width: 65,
            height: 8
        })
    ));
}
