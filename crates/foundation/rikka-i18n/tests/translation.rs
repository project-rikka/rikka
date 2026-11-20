use rikka_i18n::{
    Error, I18n, args, current_locale, generation, install, set_language, set_locale, tr,
};

fn japanese() -> I18n {
    I18n::builder()
        .source(
            "en.toml",
            "greeting = \"Hello\"\nonly_en = \"English only\"\nfiles = \"{n, plural, one {# file} other {# files}}\"",
        )
        .source("ja.toml", "greeting = \"こんにちは\"")
        .source(
            "ja-JP.toml",
            "regional = \"日本\"\n[i18n]\nlocale = \"ja-JP\"\nfallback = \"ja\"",
        )
        .fallback("en")
        .locale("ja-JP")
        .build()
        .expect("valid catalogs")
}

#[test]
fn fallback_chain_uses_own_plural_rules() {
    let i18n = japanese();
    assert_eq!(
        i18n.chain()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["ja-JP", "ja", "en"]
    );
    assert_eq!(&*i18n.tr("regional"), "日本");
    assert_eq!(&*i18n.tr("greeting"), "こんにちは");
    assert_eq!(&*i18n.tr("only_en"), "English only");
    assert_eq!(&*i18n.tr_with("files", &args![n = 1]), "1 file");
    assert_eq!(&*i18n.tr_with("files", &args![n = 3]), "3 files");
    assert_eq!(
        &*i18n.with_locale("en".parse().unwrap()).tr("greeting"),
        "Hello"
    );
}

#[test]
fn missing_key_renders_as_key() {
    assert_eq!(&*japanese().tr("nothing.here"), "nothing.here");
    assert!(!japanese().contains("nothing.here"));
    assert_eq!(&*I18n::empty().tr("app.title"), "app.title");
}

#[test]
fn later_sources_override_earlier() {
    let i18n = I18n::builder()
        .source("en.toml", "a = \"one\"\nb = \"two\"")
        .source("en.toml", "b = \"TWO\"")
        .source("ja.toml", "a = \"いち\"")
        .prefer("fr")
        .prefer("ja-JP")
        .build()
        .expect("valid catalogs");
    assert_eq!(i18n.locale().to_string(), "ja");
    assert_eq!(&*i18n.tr("a"), "いち");
    assert_eq!(&*i18n.with_locale("en".parse().unwrap()).tr("b"), "TWO");

    let embedded = I18n::builder()
        .embedded(&[
            ("en.toml", "a = \"one\"\nb = \"two\""),
            ("ja.toml", "a = \"いち\""),
        ])
        .locale("ja")
        .build()
        .expect("valid catalogs");
    assert_eq!(&*embedded.tr("b"), "two");
}

#[test]
fn broken_source_fails_build() {
    let error = I18n::builder()
        .source("en.toml", "a = 1")
        .source("ja.toml", "b = 2")
        .build()
        .unwrap_err();
    assert!(matches!(error, Error::UnexpectedValue { key, .. } if key == "a"));
    let unnamed = I18n::builder().source("strings", "a = \"x\"").build();
    assert!(matches!(unnamed, Err(Error::UnknownLocale { .. })));
}

#[test]
fn install_switches_and_bumps_generation() {
    let before = generation();
    I18n::builder()
        .source("en.toml", "greeting = \"Hello\"")
        .source("ja.toml", "greeting = \"こんにちは\"")
        .source("zh-Hans.toml", "greeting = \"你好\"")
        .locale("en")
        .install()
        .expect("valid catalogs");
    assert!(generation() > before);
    assert_eq!(&*tr("greeting"), "Hello");

    let before = generation();
    set_locale("ja".parse().expect("valid tag"));
    assert_eq!(&*tr("greeting"), "こんにちは");
    assert_eq!(current_locale().to_string(), "ja");
    assert!(generation() > before);

    assert_eq!(
        set_language("zh-CN").map(|locale| locale.to_string()),
        Some("zh-Hans".into())
    );
    assert_eq!(set_language("fr"), None);
    assert_eq!(&*tr("greeting"), "你好");

    install(I18n::empty());
    assert_eq!(&*tr("greeting"), "greeting");
}
