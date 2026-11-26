use rikka_settings::{Catalog, Error, Key, Role, Schema};

fn displays() -> Schema {
    Schema::new("rikka.displays")
        .key(Key::float("scale", 1.0).range(0.5..=4.0))
        .key(Key::bool("sync", true))
        .key(Key::bool("adaptive-sync", false))
}

fn calculator() -> Schema {
    Schema::new("com.example.calculator")
        .key(Key::int("precision", 10).range(0..=20))
        .key(Key::list("recents", []).role(Role::State))
}

#[test]
fn merged_catalog_round_trips_through_json() {
    let catalog = Catalog::merge([
        Catalog::from(calculator()),
        Catalog::new().instanced("display", displays()),
    ])
    .unwrap();
    let domains: Vec<&str> = catalog
        .entries()
        .iter()
        .map(|entry| entry.domain())
        .collect();
    assert_eq!(domains, ["com.example.calculator", "rikka.displays"]);

    let read = Catalog::from_json(&catalog.to_json()).unwrap();
    assert_eq!(read, catalog);
    let recents = read
        .entry("com.example.calculator")
        .and_then(|entry| entry.schema().key_named("recents"))
        .unwrap();
    assert!(!recents.is_preference());
}

#[test]
fn merging_refuses_a_domain_described_twice() {
    let merged = Catalog::merge([Catalog::from(calculator()), Catalog::from(calculator())]);
    assert!(
        matches!(merged, Err(Error::DuplicateDomain(domain)) if domain == "com.example.calculator")
    );
}

#[test]
fn instanced_settings_split_at_the_longest_suffix() {
    let catalog = Catalog::new().instanced("display", displays());
    let entry = catalog.entry("rikka.displays").unwrap();
    let cases = [
        (
            "dell-u2720q-1a2b-scale",
            Some(("dell-u2720q-1a2b", "scale")),
        ),
        (
            "dell-u2720q-1a2b-adaptive-sync",
            Some(("dell-u2720q-1a2b", "adaptive-sync")),
        ),
        ("dell-sync", Some(("dell", "sync"))),
        ("scale", None),
        ("-scale", None),
        ("dell-u2720q-brightness", None),
    ];
    for (name, expected) in cases {
        let found = entry
            .setting(name)
            .map(|setting| (setting.instance.unwrap(), setting.key.name()));
        assert_eq!(found, expected, "{name}");
    }
}

#[test]
fn fixed_settings_are_found_by_name() {
    let catalog = Catalog::from(calculator());
    let entry = catalog.entry("com.example.calculator").unwrap();
    let setting = entry.setting("precision").unwrap();
    assert_eq!(setting.instance, None);
    assert_eq!(setting.key.name(), "precision");
    assert!(entry.setting("missing").is_none());
}
