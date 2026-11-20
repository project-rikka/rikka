use rikka_types::{Point, Rect, Size};
use switcher_layout::{Arrangement, arrange};

const ROOM: Size = Size::new(1920.0, 1080.0);

const SLACK: f32 = 0.01;

fn cases() -> Vec<(&'static str, Vec<Size>)> {
    vec![
        ("one window", vec![Size::new(1280.0, 720.0)]),
        (
            "a few windows of every shape",
            vec![
                Size::new(1920.0, 1040.0),
                Size::new(800.0, 600.0),
                Size::new(400.0, 900.0),
                Size::new(4000.0, 200.0),
                Size::new(200.0, 2000.0),
                Size::new(0.0, 0.0),
            ],
        ),
        (
            "forty windows",
            (0..40)
                .map(|at| {
                    Size::new(
                        600.0 + (at * 37 % 900) as f32,
                        400.0 + (at * 53 % 500) as f32,
                    )
                })
                .collect(),
        ),
        ("two hundred windows", vec![Size::new(1280.0, 800.0); 200]),
    ]
}

fn within(inner: Rect, outer: Rect) -> bool {
    inner.min_x() >= outer.min_x() - SLACK
        && inner.min_y() >= outer.min_y() - SLACK
        && inner.max_x() <= outer.max_x() + SLACK
        && inner.max_y() <= outer.max_y() + SLACK
}

fn apart(one: Rect, other: Rect) -> bool {
    one.max_x() <= other.min_x() + SLACK
        || other.max_x() <= one.min_x() + SLACK
        || one.max_y() <= other.min_y() + SLACK
        || other.max_y() <= one.min_y() + SLACK
}

#[test]
fn cards_fit_panel_without_overlap() {
    for (name, windows) in cases() {
        let laid = arrange(ROOM, &windows);
        assert_eq!(
            laid.cards.len(),
            windows.len(),
            "{name}: one card per window"
        );
        assert!(
            within(laid.panel, Rect::new(0.0, 0.0, ROOM.width, ROOM.height)),
            "{name}: the panel stays on the display"
        );
        assert_eq!(
            laid.centres().iter().map(Vec::len).sum::<usize>(),
            windows.len(),
            "{name}: every card has its place in a row"
        );
        for (at, (card, window)) in laid.cards.iter().zip(&windows).enumerate() {
            assert!(
                card.frame.min_x() >= laid.viewport.min_x() - SLACK
                    && card.frame.max_x() <= laid.viewport.max_x() + SLACK,
                "{name}: card {at} stays across the panel"
            );
            if laid.reach() == 0.0 {
                assert!(
                    within(card.frame, laid.viewport),
                    "{name}: card {at} is in view"
                );
            }
            assert!(
                within(card.window, card.frame),
                "{name}: window {at} is inside its card"
            );
            assert!(
                card.window.min_y() >= card.header.max_y(),
                "{name}: window {at} is below its title"
            );
            if window.width > 0.0 && window.height > 0.0 {
                let expected = window.width / window.height;
                let shown = card.window.width() / card.window.height();
                assert!(
                    (expected - shown).abs() / expected < SLACK,
                    "{name}: window {at} keeps its shape"
                );
            }
            for (other_at, other) in laid.cards.iter().enumerate().skip(at + 1) {
                assert!(
                    apart(card.frame, other.frame),
                    "{name}: cards {at} and {other_at} do not overlap"
                );
            }
        }
    }
}

fn shown(laid: &Arrangement, selected: usize, scroll: f32) -> Rect {
    laid.cards[selected]
        .frame
        .translate(Point::new(0.0, -scroll))
}

#[test]
fn selected_card_scrolls_into_view() {
    let windows = vec![Size::new(1280.0, 800.0); 200];
    let laid = arrange(ROOM, &windows);
    assert!(laid.reach() > 0.0, "two hundred windows do not fit at once");
    let mut scroll = 0.0;
    for selected in [0, 120, 199, 3] {
        scroll = laid.scrolled_to(selected, scroll);
        let card = shown(&laid, selected, scroll);
        assert!(
            within(card, laid.viewport),
            "card {selected} is scrolled into view"
        );
        assert_eq!(laid.at(card.center(), scroll), Some(selected));
    }
    assert_eq!(
        laid.at(Point::new(laid.panel.min_x() - 1.0, 0.0), scroll),
        None
    );
}
