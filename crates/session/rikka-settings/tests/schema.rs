use rikka_settings::{Domain, Error, Key, Schema, Value};

fn calculator() -> Schema {
    Schema::new("com.example.calculator")
        .key(Key::bool("scientific", false))
        .key(Key::int("precision", 10).range(0..=20))
}

#[test]
fn rejects_unwritable_domain() {
    let schema = Schema::new("../../etc/passwd").key(Key::bool("owned", true));
    assert!(matches!(
        schema.validate().unwrap_err(),
        Error::InvalidDomain(_)
    ));
}

#[test]
fn rejects_duplicate_setting() {
    let schema = calculator().key(Key::bool("scientific", true));
    assert!(matches!(
        schema.validate().unwrap_err(),
        Error::DuplicateKey { .. }
    ));
}

#[test]
fn invalid_key_error_names_key() {
    let schema = calculator().key(Key::int("rounds", 99).range(0..=10));
    let Error::BadDeclaration { reason, .. } = schema.validate().unwrap_err() else {
        panic!("a default outside its range is a broken declaration")
    };
    assert!(reason.contains("rounds"), "{reason}");
}

#[test]
fn schema_serde_round_trip() {
    let schema = calculator().summary("The calculator's own settings").key(
        Key::string("angle-unit", "degrees")
            .summary("What an angle means")
            .choices(["degrees", "radians"]),
    );
    let document = toml::to_string(&schema).unwrap();
    let read: Schema = toml::from_str(&document).unwrap();
    assert_eq!(read, schema);
    assert!(
        read.key_named("angle-unit")
            .unwrap()
            .accepts(Value::String("gradians".into()))
            .is_err()
    );
}

#[test]
fn wrong_type_rejected_as_type_error() {
    let key = Key::int("precision", 10).range(0..=20);
    let refusal = key.accepts(Value::String("lots".into())).unwrap_err();
    assert_eq!(refusal, "expected a whole number, found some text");
}

fn domain(name: &str) -> Domain {
    name.parse().expect("test domains should be valid")
}

#[test]
fn domain_is_single_path_component() {
    for name in [
        "",
        "rikka.Appearance",
        "rikka/appearance",
        "../../etc/passwd",
        "..",
        ".appearance",
        "appearance.",
        "rikka..appearance",
        "rikka appearance",
        "rikka\\appearance",
    ] {
        assert!(
            name.parse::<Domain>().is_err(),
            "accepted {name:?} as a domain"
        );
    }
    assert!("k".repeat(129).parse::<Domain>().is_err());
}

#[test]
fn deserialized_domain_is_validated() {
    #[derive(serde::Deserialize)]
    struct Holder {
        value: Domain,
    }

    let read = toml::from_str::<Holder>("value = 'rikka.appearance'").expect("domain should parse");
    assert_eq!(read.value, domain("rikka.appearance"));
    assert!(toml::from_str::<Holder>("value = '../../etc/passwd'").is_err());
}
