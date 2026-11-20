use rikka_i18n_catalog::{Args, Message};

fn render(source: &str, args: &Args<'_>, language: &str) -> String {
    Message::parse(source)
        .expect("a valid message")
        .format(language, args)
        .to_string()
}

#[test]
fn missing_arguments_render_as_placeholders() {
    let args = Args::new().set("name", "咲").set("count", 3);
    assert_eq!(
        render("{name} さんに {count} 件", &args, "ja"),
        "咲 さんに 3 件"
    );
    assert_eq!(render("hello {name}", Args::EMPTY, "en"), "hello {name}");
}

#[test]
fn apostrophes_quote_syntax() {
    assert_eq!(
        render("'{name}' は空欄", Args::EMPTY, "en"),
        "{name} は空欄"
    );
    assert_eq!(render("don't stop", Args::EMPTY, "en"), "don't stop");
    assert_eq!(render("it''s '#'1", Args::EMPTY, "en"), "it's #1");
    assert_eq!(render("a # b", Args::EMPTY, "en"), "a # b");
}

#[test]
fn plural_and_select_nest() {
    let files = "{count, plural, =0 {none} one {# file} other {# files}}";
    let count = |count: i64, language| render(files, &Args::new().set("count", count), language);
    assert_eq!(count(0, "en"), "none");
    assert_eq!(count(1, "en"), "1 file");
    assert_eq!(count(4, "en"), "4 files");
    assert_eq!(count(1, "ja"), "1 files");

    let kinds = "{kind, select, folder {フォルダ} other {項目}}";
    assert_eq!(
        render(kinds, &Args::new().set("kind", "unknown"), "ja"),
        "項目"
    );

    let nested = "{ count , plural ,\n other {{kind, select, other {# 個の {kind}}}}}";
    let args = Args::new().set("count", 2).set("kind", "file");
    assert_eq!(render(nested, &args, "en"), "2 個の file");
}

#[test]
fn malformed_messages_are_refused() {
    for source in [
        "{}",
        "{name",
        "unmatched }",
        "{count, plural, one {#}}",
        "{count, plural, huge {#} other {#}}",
        "{kind, select, folder {x}}",
        "{count, unknown, other {x}}",
    ] {
        assert!(
            Message::parse(source).is_err(),
            "`{source}` should not parse"
        );
    }
}
