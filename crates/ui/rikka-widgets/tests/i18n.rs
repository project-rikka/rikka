use std::time::Instant;

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::Size;
use rikka_ui::*;
use rikka_widgets::column;

const EN: &str = r#"
[i18n]
locale = "en"

[settings]
title = "Settings"
items = "{count, plural, one {# item} other {# items}}"
"#;

const JA: &str = r#"
[i18n]
locale = "ja"

[settings]
title = "設定"
"#;

fn drawn(text: Text) -> String {
    let theme = Theme::dark();
    match with_theme(&theme, || Widget::measure(&text)) {
        Some(MeasureSpec::Text { run, .. }) => run.content.to_string(),
        other => panic!("text should measure as text, got {other:?}"),
    }
}

#[derive(State)]
struct Screen;

fn view(_screen: &Screen) -> Element {
    column! { text(tr!("settings.title")) }.build()
}

#[test]
fn keys_resolve_through_installed_catalogs() {
    assert_eq!(
        drawn(text(tr!("settings.title"))),
        "settings.title",
        "with no catalogs a key is drawn as itself"
    );

    I18n::builder()
        .source("en.toml", EN)
        .source("ja.toml", JA)
        .fallback("en")
        .locale("ja")
        .install()
        .expect("valid catalogs");

    assert_eq!(drawn(text(tr!("settings.title"))), "設定");
    assert_eq!(
        drawn(text(tr!("settings.items"))),
        "{count}",
        "missing arguments not reported"
    );
    assert_eq!(
        drawn(text("いつもの文章")),
        "いつもの文章",
        "plain string altered"
    );
    assert_eq!(
        drawn(text("settings.title")),
        "settings.title",
        "plain string looked up as key"
    );
    assert_eq!(
        Label::from("settings.title").as_str(),
        "settings.title",
        "label from string not verbatim"
    );
    assert_eq!(
        Label::from(String::from("settings.title")).as_str(),
        "settings.title"
    );
    assert_eq!(
        Label::translated("settings.title").as_str(),
        "設定",
        "untranslated label treated as key"
    );

    assert_eq!(&*tr!("settings.items", count = 1), "1 item");
    assert_eq!(&*tr!("settings.items", count = 4), "4 items");

    let mut scene = Scene::new();
    let mut ui = Ui::new(Screen, view, Theme::dark());
    ui.set_viewport(Size::new(200.0, 100.0), 1.0);
    let now = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(now));
    assert!(!ui.needs_render(), "first frame did not settle");

    i18n::set_locale("en".parse().expect("valid tag"));
    assert!(ui.needs_render(), "locale change requested no frame");
    ui.render(Rendering::new(&mut scene).with_time(now));
    assert!(!ui.needs_render());

    i18n::install(I18n::empty());
    assert_eq!(drawn(text(tr!("settings.title"))), "settings.title");
}
