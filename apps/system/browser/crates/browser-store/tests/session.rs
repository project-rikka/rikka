use std::sync::Arc;

use browser_store::{
    PinnedPlace, Session, SessionSplit, SessionTab, SessionWindow, SpaceId, SpaceSession,
    SplitAxis, SplitMember, Store, TabRecord, Timestamp, WindowBounds,
};

use crate::support;

fn tab(store: &Store, url: &str, state: Option<Vec<u8>>) -> SessionTab {
    SessionTab {
        id: store.new_tab_id(),
        pinned: None,
        favorite: None,
        url: url.to_owned(),
        title: url.to_owned(),
        state: state.map(Arc::from),
        last_active: Timestamp::from_unix_millis(1_700_000_000_000),
        split: None,
    }
}

fn session(space: SpaceId, tabs: Vec<SessionTab>, active: usize) -> Session {
    Session {
        windows: vec![SessionWindow {
            active_space: Some(space),
            bounds: Some(WindowBounds {
                position: None,
                width: 1280,
                height: 800,
            }),
            maximized: true,
            spaces: vec![SpaceSession {
                space,
                active: tabs.get(active).map(|tab| tab.id),
                tabs,
            }],
            splits: Vec::new(),
        }],
    }
}

#[test]
fn session_round_trips() {
    let store = Store::in_memory().unwrap();
    let (_, space) = support::space(&store, "Home");
    let pinned = store.pin_tab(PinnedPlace::root(space), "https://mail.example/", None);
    let group = store.new_split_group_id();
    let mut live = tab(&store, "https://mail.example/inbox", Some(vec![7; 40_000]));
    live.pinned = Some(pinned);
    let mut left = tab(&store, "https://a.example/", Some(vec![1, 2, 3]));
    left.split = Some(SplitMember { group, order: 0 });
    let mut right = tab(&store, "https://b.example/", None);
    right.split = Some(SplitMember { group, order: 1 });
    let mut saved = session(space, vec![live, left, right], 1);
    saved.windows[0].splits.push(SessionSplit {
        group,
        axis: SplitAxis::Horizontal,
        shares: vec![0.25, 0.75],
    });

    store.save_session(saved.clone());
    store.flush().unwrap();
    assert_eq!(store.load_session().unwrap(), saved);
}

#[test]
fn session_survives_reopen() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("browser.sqlite");
    let store = Store::open(&path).unwrap();
    let (_, space) = support::space(&store, "Home");
    let saved = session(
        space,
        vec![tab(&store, "https://a.example/", Some(vec![5; 64]))],
        0,
    );
    store.save_session(saved.clone());
    drop(store);

    let store = Store::open(&path).unwrap();
    assert_eq!(store.load_session().unwrap(), saved);
    assert!(store.new_tab_id() > saved.windows[0].spaces[0].tabs[0].id);
}

fn member(tab: &mut SessionTab, group: browser_store::SplitGroupId, order: u32) {
    tab.split = Some(SplitMember { group, order });
}

#[test]
fn split_groups_are_validated_and_normalized() {
    let store = Store::in_memory().unwrap();
    let (profile, home) = support::space(&store, "Home");
    let work = store.create_space(browser_store::SpaceFields {
        name: "Work".to_owned(),
        icon: browser_store::SpaceIcon::Symbol("work".to_owned()),
        profile,
    });
    let [intact, lonely, negative, unrecorded, empty, removed] =
        std::array::from_fn(|_| store.new_split_group_id());
    let mut tabs: Vec<SessionTab> = (0..6)
        .map(|index| tab(&store, &format!("https://t{index}.example/"), None))
        .collect();
    member(&mut tabs[0], intact, 1);
    member(&mut tabs[1], intact, 0);
    member(&mut tabs[2], lonely, 0);
    member(&mut tabs[3], negative, 0);
    member(&mut tabs[4], negative, 1);
    member(&mut tabs[5], unrecorded, 0);
    let mut work_tabs: Vec<SessionTab> = (0..2)
        .map(|index| tab(&store, &format!("https://w{index}.example/"), None))
        .collect();
    member(&mut work_tabs[0], removed, 0);
    member(&mut work_tabs[1], removed, 1);

    let mut saved = session(home, tabs, 0);
    saved.windows[0].spaces.push(SpaceSession {
        space: work,
        active: None,
        tabs: work_tabs,
    });
    let split = |group, shares: &[f32]| SessionSplit {
        group,
        axis: SplitAxis::Vertical,
        shares: shares.to_vec(),
    };
    saved.windows[0].splits = vec![
        split(intact, &[1.0, 3.0]),
        split(lonely, &[1.0, 1.0]),
        split(negative, &[1.0, -1.0]),
        split(empty, &[1.0, 1.0]),
        split(removed, &[1.0, 1.0]),
        split(intact, &[2.0, 2.0]),
    ];
    store.save_session(saved);
    store.flush().unwrap();

    let window = &store.load_session().unwrap().windows[0];
    assert_eq!(
        window.splits,
        [split(intact, &[0.25, 0.75]), split(removed, &[0.5, 0.5])]
    );
    let members: Vec<_> = window.spaces[0].tabs.iter().map(|tab| tab.split).collect();
    assert_eq!(
        members,
        [
            Some(SplitMember {
                group: intact,
                order: 1
            }),
            Some(SplitMember {
                group: intact,
                order: 0
            }),
            None,
            None,
            None,
            None,
        ]
    );

    store.remove_space(work);
    store.flush().unwrap();
    let window = &store.load_session().unwrap().windows[0];
    assert_eq!(window.splits, [split(intact, &[0.25, 0.75])]);
}

#[test]
fn closed_tab_pops_without_flush() {
    let store = Store::in_memory().unwrap();
    let (_, space) = support::space(&store, "Home");
    for url in ["https://first.example/", "https://second.example/"] {
        store.push_closed_tab(
            space,
            TabRecord {
                url: url.to_owned(),
                title: url.to_owned(),
                state: None,
            },
        );
    }

    let popped: Vec<String> = std::iter::from_fn(|| store.pop_closed_tab().unwrap())
        .map(|(closed, restored)| {
            assert_eq!(closed.url, restored.tab.url);
            assert_eq!(restored.space, space);
            restored.tab.url
        })
        .collect();
    assert_eq!(
        popped,
        ["https://second.example/", "https://first.example/"]
    );
}
