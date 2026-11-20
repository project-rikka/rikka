use rikka_computer::Display;

#[test]
fn capture_size_fits_default_and_keeps_aspect() {
    for ((width, height), shot) in [
        ((1280, 800), (1280, 800)),
        ((1024, 768), (1024, 768)),
        ((2560, 1600), (1280, 800)),
        ((1920, 1080), (1280, 720)),
    ] {
        assert_eq!(
            Display::new(width, height).capture_size(),
            shot,
            "{width}x{height}"
        );
    }
}

#[test]
fn display_size_is_clamped() {
    assert_eq!(Display::new(0, 0), Display::new(320, 240));
    assert_eq!(Display::new(99_999, 99_999), Display::new(3840, 2160));
}
