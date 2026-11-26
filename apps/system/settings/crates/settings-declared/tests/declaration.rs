use rikka_nix::{Document, Expr, Module};
use rikka_settings::{Source, Value, Values};
use settings_declared::{Setting, SettingId, declares, expr, path};

fn setting(domain: &str, instance: Option<&str>, key: &str, value: Value) -> Setting {
    let name = match instance {
        Some(instance) => format!("{instance}-{key}"),
        None => key.to_owned(),
    };
    Setting {
        id: SettingId {
            domain: domain.to_owned(),
            name,
        },
        instance: instance.map(str::to_owned),
        key: key.to_owned(),
        summary: None,
        value: value.clone(),
        source: Source::User,
        user: Some(value),
        beneath: Value::Bool(false),
        beneath_source: Source::Default,
    }
}

#[test]
fn declarations_land_under_the_users_settings() {
    let place = Values::from([
        ("name".to_owned(), Value::String("Kyoto".to_owned())),
        ("latitude".to_owned(), Value::Float(35.01)),
    ]);
    let settings = [
        setting(
            "rikka.windows",
            None,
            "layout",
            Value::String("tiling".to_owned()),
        ),
        setting("rikka.windows", None, "gap-inner", Value::Int(8)),
        setting(
            "rikka.displays",
            Some("dell-u2720q-1a2b"),
            "scale",
            Value::Float(1.5),
        ),
        setting(
            "rikka.ime",
            None,
            "sources",
            Value::List(vec![
                Value::String("keyboard-us".to_owned()),
                Value::String("mozc".to_owned()),
            ]),
        ),
        setting("rikka.weather", None, "place", Value::Table(place)),
    ];

    let mut document = Module::Settings.blank();
    for setting in &settings {
        document
            .set(path("alice", setting).unwrap(), expr(&setting.value))
            .unwrap();
    }
    let text = document.to_string();
    rikka_test_nixfmt::assert_formatted(&text);
    assert!(
        text.contains("\"rikka.windows\""),
        "domains are quoted attribute names: {text}"
    );

    let read = Document::parse(&text).unwrap();
    for setting in &settings {
        assert_eq!(
            declares(&read, "alice", setting),
            Some(expr(&setting.value)),
            "{}",
            setting.id.name
        );
        assert_eq!(declares(&read, "bob", setting), None);
    }
}

#[test]
fn values_keep_their_kind() {
    assert_eq!(expr(&Value::Float(2.0)), Expr::Float(2.0));
    assert_eq!(expr(&Value::Int(2)), Expr::Int(2));
    assert_eq!(expr(&Value::Bool(true)), Expr::Bool(true));
}
