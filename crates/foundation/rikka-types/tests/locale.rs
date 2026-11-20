use rikka_types::LocaleCategory;

type Environment = &'static [(&'static str, &'static str)];

fn tag(category: LocaleCategory, environment: Environment) -> Option<String> {
    category
        .locale_with(|variable| {
            environment
                .iter()
                .find(|(name, _)| *name == variable)
                .map(|(_, value)| (*value).to_owned())
        })
        .map(|locale| locale.tag())
}

#[test]
fn category_follows_posix_precedence() {
    let cases: [(Environment, LocaleCategory, Option<&str>); 7] = [
        (
            &[("LANG", "ja_JP.UTF-8"), ("LC_MESSAGES", "en_US.UTF-8")],
            LocaleCategory::Messages,
            Some("en-US"),
        ),
        (
            &[("LANG", "ja_JP.UTF-8"), ("LC_MESSAGES", "en_US.UTF-8")],
            LocaleCategory::Time,
            Some("ja-JP"),
        ),
        (
            &[("LC_ALL", "de_DE@euro"), ("LC_MESSAGES", "en_US.UTF-8")],
            LocaleCategory::Messages,
            Some("de-DE"),
        ),
        (
            &[("LC_ALL", ""), ("LANG", "fr_CA.UTF-8")],
            LocaleCategory::Measurement,
            Some("fr-CA"),
        ),
        (
            &[("LC_ALL", "C"), ("LANG", "fr_CA.UTF-8")],
            LocaleCategory::Messages,
            None,
        ),
        (&[("LANG", "C.UTF-8")], LocaleCategory::Ctype, None),
        (
            &[("LANG", "sr_RS@latin")],
            LocaleCategory::Ctype,
            Some("sr-RS"),
        ),
    ];
    for (environment, category, expected) in cases {
        assert_eq!(
            tag(category, environment).as_deref(),
            expected,
            "{environment:?} {category:?}"
        );
    }
}
