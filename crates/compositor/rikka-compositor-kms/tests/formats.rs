use rikka_compositor_types::{Format, FormatSet, Fourcc, Modifier};

#[test]
fn intersection_matches_format_and_modifier() {
    let display: FormatSet = [
        Format::new(Fourcc::XRGB8888, Modifier(5)),
        Format::new(Fourcc::XRGB8888, Modifier::LINEAR),
        Format::new(Fourcc::XRGB8888, Modifier(5)),
    ]
    .into_iter()
    .collect();
    let renderer: FormatSet = [
        Format::new(Fourcc::XRGB8888, Modifier(5)),
        Format::new(Fourcc::ARGB8888, Modifier::LINEAR),
    ]
    .into_iter()
    .collect();

    assert_eq!(
        display.modifiers(Fourcc::XRGB8888).collect::<Vec<_>>(),
        vec![Modifier::LINEAR, Modifier(5)]
    );
    let shared = display.intersection(&renderer);
    assert_eq!(
        shared.iter().collect::<Vec<_>>(),
        vec![Format::new(Fourcc::XRGB8888, Modifier(5))]
    );
    assert!(!shared.supports(Fourcc::ARGB8888));
}
