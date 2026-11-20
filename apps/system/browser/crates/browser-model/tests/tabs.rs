use browser_model::{LivePage, Loaded, Model, Names, Placement};
use browser_store::Store;

#[derive(Debug)]
struct FakePage(u32);

impl LivePage for FakePage {
    type View = u32;

    fn view(&self) -> &u32 {
        &self.0
    }
}

fn name() -> String {
    "Default".to_owned()
}

fn numbered(number: usize) -> String {
    format!("Default {number}")
}

const NAMES: Names = Names {
    profile: name,
    numbered_profile: numbered,
    space: name,
    numbered_space: numbered,
    folder: name,
};

fn model() -> Model<FakePage> {
    let store = Store::in_memory().expect("in-memory store");
    let loaded = Loaded::read(&store).expect("empty store reads");
    Model::load(store, &loaded, NAMES)
}

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
fn closing_hands_back_only_live_views() {
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

    let closing = model
        .close_tab(window, live)
        .expect("a live tab leaves a view");
    assert_eq!(closing.view, 7);
    assert!(model.close_tab(window, unloaded).is_none());
    assert!(model.tab(live).is_none() && model.tab(unloaded).is_none());
}
