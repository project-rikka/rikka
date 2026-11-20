use rikka_omnibox::{Error, SearchEngine, SearchEngines};

#[test]
fn builds_and_validates_custom_engines() {
    let engine = SearchEngine::builder("mdn")
        .name(" MDN ")
        .keyword("MDN")
        .search_url("https://developer.mozilla.org/search?q=%s")
        .suggestion_url("https://developer.mozilla.org/suggest?q={searchTerms}")
        .build()
        .unwrap();
    assert_eq!(engine.name(), "MDN");
    assert_eq!(engine.keyword(), "mdn");
    assert_eq!(engine.home_host(), "developer.mozilla.org");
    assert!(!engine.is_builtin());
    assert_eq!(
        engine.search_url("array map").as_str(),
        "https://developer.mozilla.org/search?q=array+map"
    );
    let path = SearchEngine::builder("wiki")
        .name("Wiki")
        .keyword("wk")
        .search_url("https://wiki.example/w/%s")
        .build()
        .unwrap();
    assert_eq!(
        path.search_url("a b/c").as_str(),
        "https://wiki.example/w/a%20b%2Fc"
    );
    assert_eq!(path.suggestion_url("x"), None);

    let base = || {
        SearchEngine::builder("x")
            .name("X")
            .keyword("x")
            .search_url("https://x.example/?q=%s")
    };
    assert!(base().build().is_ok());
    assert!(matches!(
        SearchEngine::builder(" ")
            .name("X")
            .keyword("x")
            .search_url("https://x.example/?q=%s")
            .build(),
        Err(Error::MissingId)
    ));
    assert!(matches!(base().name("").build(), Err(Error::MissingName)));
    assert!(matches!(
        base().keyword("two words").build(),
        Err(Error::InvalidKeyword(_))
    ));
    assert!(matches!(
        base().keyword("").build(),
        Err(Error::InvalidKeyword(_))
    ));
    assert!(matches!(
        base().search_url("").build(),
        Err(Error::MissingTemplate)
    ));
    assert!(matches!(
        base().search_url("https://x.example/").build(),
        Err(Error::MissingPlaceholder(_))
    ));
    assert!(matches!(
        base().search_url("javascript:%s").build(),
        Err(Error::InvalidTemplate(_))
    ));
    assert!(matches!(
        base().suggestion_url("file:///%s").build(),
        Err(Error::InvalidTemplate(_))
    ));
    assert!(
        base()
            .suggestion_url("  ")
            .build()
            .unwrap()
            .suggestion_url("q")
            .is_none()
    );
}

#[test]
fn edits_engine_list() {
    let mut engines = SearchEngines::builtin();
    let custom = |id: &str, keyword: &str| {
        SearchEngine::builder(id)
            .name(id)
            .keyword(keyword)
            .search_url("https://x.example/?q=%s")
            .build()
            .unwrap()
    };
    engines.add(custom("x", "x")).unwrap();
    assert!(matches!(
        engines.add(custom("x", "y")),
        Err(Error::DuplicateId(_))
    ));
    assert!(matches!(
        engines.add(custom("y", "@DDG")),
        Err(Error::DuplicateKeyword(_))
    ));
    engines.set_default("x").unwrap();
    assert_eq!(engines.default_engine().id(), "x");
    assert!(matches!(
        engines.set_default("nope"),
        Err(Error::UnknownEngine(_))
    ));
    assert!(matches!(
        engines.remove("bing"),
        Err(Error::BuiltinEngine(_))
    ));
    assert_eq!(engines.remove("x").unwrap().id(), "x");
    assert_eq!(engines.default_engine().id(), "google");
    assert!(matches!(engines.remove("x"), Err(Error::UnknownEngine(_))));
    engines.set_default("yahoo-japan").unwrap();
    engines.add(custom("z", "z")).unwrap();
    engines.remove("z").unwrap();
    assert_eq!(engines.default_engine().id(), "yahoo-japan");
}
