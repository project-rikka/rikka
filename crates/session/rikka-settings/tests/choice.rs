use rikka_settings::{Key, Layers, Schema, Store};
use rikka_types::Choice;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Angle {
    #[default]
    Degrees,
    Radians,
}

impl Choice for Angle {
    const ALL: &'static [Angle] = &[Angle::Degrees, Angle::Radians];

    fn slug(self) -> &'static str {
        match self {
            Angle::Degrees => "degrees",
            Angle::Radians => "radians",
        }
    }
}

fn schema() -> Schema {
    Schema::new("com.example.calculator").key(Key::choice::<Angle>("angle-unit"))
}

#[test]
fn choice_defaults_serializes_and_rejects_unknown() {
    let system = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let layers = Layers::new(system.path(), user.path());
    let (mut store, _) = Store::load(schema(), layers.clone()).unwrap();

    assert_eq!(store.choice::<Angle>("angle-unit"), Angle::Degrees);
    store.set_choice("angle-unit", Angle::Radians).unwrap();
    assert_eq!(store.choice::<Angle>("angle-unit"), Angle::Radians);
    assert_eq!(store.get::<String>("angle-unit").unwrap(), "radians");
    assert!(store.set("angle-unit", "gradians").is_err());

    rikka_settings::flush_pending_writes();
    std::fs::write(
        user.path().join("com.example.calculator.toml"),
        "angle-unit = \"gradians\"\n",
    )
    .unwrap();
    let (store, _) = Store::load(schema(), layers).unwrap();
    assert_eq!(store.choice::<Angle>("angle-unit"), Angle::Degrees);
    assert_eq!(Angle::from_slug("gradians"), None);
    assert_eq!(Angle::of_slug("radians"), Angle::Radians);
}
