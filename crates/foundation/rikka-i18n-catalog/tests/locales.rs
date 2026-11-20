use rikka_i18n_catalog::{Locale, negotiate};

fn locale(tag: &str) -> Locale {
    tag.parse().expect("valid tag")
}

#[test]
fn normalizes_tags_rejects_missing_language() {
    assert_eq!(locale("JA_jp").to_string(), "ja-JP");
    assert_eq!(locale("zh-hant-tw").to_string(), "zh-Hant-TW");
    assert_eq!(locale("es-419").to_string(), "es-419");
    assert_eq!(locale("ja_JP.UTF-8").to_string(), "ja-JP");
    assert_eq!(locale("de-DE-1901-u-co-phonebk").to_string(), "de-DE");
    for tag in ["", "-", "123", "toolong", "e"] {
        assert!(tag.parse::<Locale>().is_err(), "{tag} should not parse");
    }
}

#[test]
fn negotiate_prefers_exact_then_ancestor() {
    let available = [locale("en"), locale("ja"), locale("pt-BR")];
    let pick = |tag: &str| negotiate([&locale(tag)], &available).map(|l| l.to_string());
    assert_eq!(pick("ja-JP"), Some("ja".into()));
    assert_eq!(pick("pt"), Some("pt-BR".into()));
    assert_eq!(pick("pt-PT"), Some("pt-BR".into()));
    assert_eq!(pick("fr"), None);

    let requested = [locale("fr"), locale("ja"), locale("en")];
    assert_eq!(
        negotiate(&requested, &available).map(|l| l.to_string()),
        Some("ja".into())
    );
}
