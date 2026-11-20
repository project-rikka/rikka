use rikka_languages::Language;

#[test]
fn lookup_falls_back_to_listed_tags() {
    let cases = [
        ("en-US", Some("en")),
        ("EN_gb", Some("en")),
        ("es-419", Some("es")),
        ("es-MX", Some("es-MX")),
        ("pt-BR", Some("pt")),
        ("pt-PT", Some("pt-PT")),
        ("sr-Latn-RS", Some("sr-Latn")),
        ("zh", Some("zh-Hans")),
        ("zh-CN", Some("zh-Hans")),
        ("zh-Hans-SG", Some("zh-Hans")),
        ("zh-TW", Some("zh-Hant")),
        ("zh-Hant-HK", Some("zh-Hant")),
        ("cmn-Hant", Some("zh-Hant")),
        ("zh-yue", Some("yue")),
        ("iw", Some("he")),
        ("in", Some("id")),
        ("tl", Some("fil")),
        ("no", Some("nb")),
        ("fa-AF", Some("prs")),
        ("mni", Some("mni-Mtei")),
        ("", None),
        ("x", None),
        ("english", None),
        ("en US", None),
        ("qqq", None),
    ];
    for (tag, expected) in cases {
        assert_eq!(Language::lookup(tag).map(Language::tag), expected, "{tag}");
    }
}

#[test]
fn every_listed_tag_looks_up_to_itself() {
    for language in Language::all() {
        assert_eq!(Language::lookup(language.tag()), Some(language));
        assert_eq!(
            Language::from_tag(&language.tag().to_ascii_uppercase()),
            Some(language)
        );
    }
}
