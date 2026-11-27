use browser_model::Placement;

use crate::support::{FakePage, model};

#[test]
fn opened_tabs_land_after_their_opener() {
    let mut model = model();
    let window = model.add_window(false);
    let first = model
        .open_tab(
            window,
            Some("https://a.example/".into()),
            Placement::Top,
            true,
        )
        .expect("first tab");
    let third = model
        .open_tab(
            window,
            Some("https://c.example/".into()),
            Placement::After(first),
            false,
        )
        .expect("third tab");
    let second = model
        .open_tab(
            window,
            Some("https://b.example/".into()),
            Placement::After(first),
            false,
        )
        .expect("second tab");

    assert_eq!(model.strip(window), [first, second, third]);
    assert_eq!(
        model.window(window).and_then(|layout| layout.active()),
        Some(first)
    );
    assert_eq!(model.profiles.list()[0].name, "Default");
}

#[test]
fn closing_archives_tabs_and_hands_back_only_live_views() {
    let mut model = model();
    let window = model.add_window(false);
    let unloaded = model
        .open_tab(
            window,
            Some("https://a.example/".into()),
            Placement::Top,
            true,
        )
        .expect("unloaded tab");
    let live = model
        .adopt_tab(
            window,
            FakePage(7),
            "https://b.example/".into(),
            Placement::After(unloaded),
            true,
        )
        .expect("live tab");

    assert_eq!(model.close_tab(window, live), Some(7));
    assert_eq!(model.close_tab(window, unloaded), None);
    assert!(model.tab(live).is_none() && model.tab(unloaded).is_none());
    model.store().flush().expect("store writes");
    let closed: Vec<String> = model
        .store()
        .closed_tabs()
        .expect("closed tabs")
        .into_iter()
        .map(|closed| closed.url)
        .collect();
    assert_eq!(closed, ["https://a.example/", "https://b.example/"]);
}
