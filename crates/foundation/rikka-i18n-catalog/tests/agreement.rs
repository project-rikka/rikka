use rikka_i18n_catalog::{Error, verify_catalogs};

const ENGLISH: (&str, &str) = ("en.toml", "[app]\ntitle = \"Rikka\"\nquit = \"Quit\"");

#[test]
fn matching_catalogs_agree() {
    let japanese = ("ja.toml", "[app]\ntitle = \"六花\"\nquit = \"終了\"");
    let chinese = ("zh-Hans.toml", "app.title = \"六花\"\napp.quit = \"退出\"");
    assert!(verify_catalogs(&[ENGLISH, japanese, chinese]).is_ok());
}

#[test]
fn every_disagreement_is_reported() {
    type Sources = &'static [(&'static str, &'static str)];
    type Expectation = fn(&[Error]) -> bool;
    let cases: &[(&str, Sources, Expectation)] = &[
        (
            "missing and unexpected keys",
            &[
                ENGLISH,
                ("ja.toml", "[app]\ntitle = \"六花\"\nhelp = \"助け\""),
            ],
            |errors| {
                matches!(errors, [Error::Disagrees { source_name, reference, missing, unexpected }]
                    if source_name == "ja.toml"
                        && reference == "en.toml"
                        && missing == &["app.quit"]
                        && unexpected == &["app.help"])
            },
        ),
        (
            "no English reference",
            &[("ja.toml", "title = \"六花\"")],
            |errors| matches!(errors, [Error::NoReference]),
        ),
        (
            "an empty reference",
            &[("en.toml", ""), ("ja.toml", "")],
            |errors| matches!(errors, [Error::EmptyReference { source_name }] if source_name == "en.toml"),
        ),
        (
            "a locale the file name does not carry",
            &[
                ENGLISH,
                (
                    "ja.toml",
                    "[i18n]\nlocale = \"ko\"\n[app]\ntitle = \"릭카\"\nquit = \"종료\"",
                ),
            ],
            |errors| {
                matches!(errors, [Error::MisnamedLocale { source_name, locale }]
                    if source_name == "ja.toml" && locale.to_string() == "ko")
            },
        ),
        (
            "a malformed message beside a disagreeing catalog",
            &[
                ENGLISH,
                (
                    "ja.toml",
                    "[app]\ntitle = \"{n, plural, one {#}}\"\nquit = \"終了\"",
                ),
                ("ru.toml", "[app]\ntitle = \"Рикка\""),
            ],
            |errors| {
                matches!(errors, [Error::Message { source_name, .. }, Error::Disagrees { source_name: other, .. }]
                    if source_name == "ja.toml" && other == "ru.toml")
            },
        ),
    ];

    for (case, sources, expected) in cases {
        let errors = verify_catalogs(sources).expect_err(case);
        assert!(expected(&errors), "{case}: {errors:?}");
    }
}
