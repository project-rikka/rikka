use rikka_i18n_catalog::{Catalog, Error, Message};

#[test]
fn flattens_nested_tables() {
    let source = r#"
        [i18n]
        locale = "ja"
        name = "日本語"
        fallback = "en"

        [app]
        title = "六花"

        [app.window]
        close = "閉じる"
    "#;
    let catalog = Catalog::parse("messages", source).expect("valid catalog");
    assert_eq!(catalog.locale().to_string(), "ja");
    assert_eq!(catalog.display_name(), "日本語");
    assert_eq!(
        catalog.fallback().map(ToString::to_string),
        Some("en".into())
    );
    assert_eq!(catalog.len(), 2);
    assert_eq!(
        catalog
            .get("app.window.close")
            .and_then(Message::as_literal)
            .map(|t| &**t),
        Some("閉じる")
    );

    let named = Catalog::parse("locales/ja-JP.toml", "greeting = \"やあ\"").expect("valid catalog");
    assert_eq!(named.locale().to_string(), "ja-JP");
}

#[test]
fn reports_catalog_errors() {
    assert!(matches!(
        Catalog::parse("messages", "greeting = \"hi\"").unwrap_err(),
        Error::UnknownLocale { .. }
    ));
    let error = Catalog::parse("messages", "[i18n]\nlocale = \"en\"\n[app]\nsize = 3").unwrap_err();
    assert!(matches!(error, Error::UnexpectedValue { key, .. } if key == "app.size"));
    let source = "[i18n]\nlocale = \"en\"\n[app]\nbad = \"{count, plural, one {#}}\"";
    assert!(matches!(
        Catalog::parse("messages", source).unwrap_err(),
        Error::Message { key, .. } if key == "app.bad"
    ));
}
