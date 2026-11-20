use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Rect, Size};
use rikka_ui::*;
use rikka_widgets::*;

use crate::common::dump_rect;

const VIEW: Size = Size::new(760.0, 300.0);
const NARROW: f32 = 240.0;
const LABEL: f32 = 12.0;
const GLYPH: f32 = 15.0;
const SLACK: f32 = 1.0;
const FINGERPRINT: &str = "7802:9f8b:f82d:b932:b7eb:4c46:d479:b334:222f:ae25:2218:902c:942c:1b2c";
const SETTING: &str = "Notifications";
const DESCRIPTION: &str = "Show banners for messages, calls and reminders from every app";

#[derive(State)]
struct Card {
    room: f32,
}

fn card(card: &Card) -> Element {
    column_of([
        container(
            row! {
                icon(MaterialSymbol::KEY).extent(GLYPH);
                column_of([
                    text("Fingerprint").text_size(LABEL).any(),
                    text(FINGERPRINT).any(),
                ])
                .grow(1.0);
                button(text("Copy")).ghost().small().on_press(|| {});
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width(),
        )
        .width(card.room)
        .any(),
        container(
            row! {
                text(SETTING);
                text(DESCRIPTION);
            }
            .gap(Space::SNUG)
            .align_items(Align::Start)
            .fill_width(),
        )
        .width(card.room)
        .any(),
    ])
    .fill()
    .build()
}

fn dump_at(room: f32) -> String {
    let mut ui = TypedUi::new(Card { room }, card, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut Scene::new());
    ui.dump()
}

fn painted_text(dump: &str, content: &str) -> Rect {
    let quoted = format!("{content:?}");
    dump.lines()
        .find(|line| line.contains("· text") && line.trim_end().ends_with(&quoted))
        .map(dump_rect)
        .unwrap_or_else(|| panic!("{content} is painted"))
}

#[test]
fn long_words_break_only_where_nothing_else_fits() {
    let wide = dump_at(VIEW.width);
    let narrow = dump_at(NARROW);

    let fingerprint = painted_text(&narrow, FINGERPRINT);
    let copy = painted_text(&narrow, "Copy");
    assert!(
        fingerprint.max_x() <= copy.x() + SLACK,
        "fingerprint {fingerprint:?} runs under {copy:?}"
    );
    assert!(
        copy.max_x() <= NARROW + SLACK,
        "copy {copy:?} outside the card"
    );
    assert!(
        fingerprint.height() > painted_text(&wide, FINGERPRINT).height(),
        "fingerprint {fingerprint:?} is not wrapped"
    );

    let setting = painted_text(&narrow, SETTING);
    let whole = painted_text(&wide, SETTING);
    let description = painted_text(&narrow, DESCRIPTION);
    assert_eq!(
        [setting.width(), setting.height()],
        [whole.width(), whole.height()],
        "{SETTING} is broken inside the word"
    );
    assert!(
        description.x() >= setting.max_x() && description.max_x() <= NARROW + SLACK,
        "description {description:?} beside {setting:?} leaves the card"
    );
}
