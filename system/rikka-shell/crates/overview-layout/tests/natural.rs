use overview_layout::{Spacing, arrange};
use rikka_types::Rect;

const AREA: Rect = Rect::new(28.0, 72.0, 1300.0, 620.0);

const SPACING: Spacing = Spacing {
    row: 44.0,
    column: 28.0,
};

const SCREEN: f32 = 768.0;

const SLACK: f32 = 0.01;

const LARGEST: f32 = 0.95;
fn cases() -> Vec<(&'static str, Vec<Rect>)> {
    vec![
        (
            "one small window",
            vec![Rect::new(500.0, 300.0, 320.0, 180.0)],
        ),
        (
            "one maximized window",
            vec![Rect::new(0.0, 32.0, 1366.0, 736.0)],
        ),
        (
            "a tall window beside a wide one",
            vec![
                Rect::new(40.0, 40.0, 300.0, 700.0),
                Rect::new(400.0, 200.0, 900.0, 400.0),
            ],
        ),
        (
            "windows stacked on one spot",
            vec![Rect::new(200.0, 100.0, 800.0, 500.0); 5],
        ),
        (
            "a dozen windows of every size",
            (0..12)
                .map(|at| {
                    let at = at as f32;
                    Rect::new(
                        at * 97.0 % 900.0,
                        at * 53.0 % 400.0,
                        180.0 + at * 61.0 % 700.0,
                        120.0 + at * 37.0 % 500.0,
                    )
                })
                .collect(),
        ),
    ]
}

#[test]
fn windows_keep_aspect_without_overlap() {
    for (name, windows) in cases() {
        let slots = arrange(&windows, AREA, SPACING, SCREEN);
        assert_eq!(slots.len(), windows.len(), "{name}: one slot per window");
        for (window, slot) in windows.iter().zip(&slots) {
            let shape = window.width() / window.height();
            let aspect = slot.width() / slot.height();
            assert!(
                (shape - aspect).abs() <= shape * SLACK,
                "{name}: {window:?} became {slot:?}"
            );
            assert!(
                slot.width() <= window.width() * LARGEST * (1.0 + SLACK),
                "{name}: {window:?} grew to {slot:?}"
            );
            assert!(
                AREA.inflate(1.0).contains_rect(slot),
                "{name}: {slot:?} leaves the area"
            );
        }
        let apart = SPACING.row.min(SPACING.column) / 2.0 - 1.0;
        for (at, one) in slots.iter().enumerate() {
            for other in &slots[at + 1..] {
                assert!(
                    one.inflate(apart)
                        .intersection(&other.inflate(apart))
                        .is_none(),
                    "{name}: {one:?} crowds {other:?}"
                );
            }
        }
    }
}

#[test]
fn empty_input_arranges_nothing() {
    assert!(arrange(&[], AREA, SPACING, SCREEN).is_empty());
    let window = [Rect::new(0.0, 0.0, 400.0, 300.0)];
    assert!(arrange(&window, Rect::new(0.0, 0.0, 0.0, 300.0), SPACING, SCREEN).is_empty());
}
