use rikka_types::{Color, srgb};

#[test]
fn parses_hex_forms_and_rejects_malformed() {
    assert_eq!(Color::parse_hex("#fff"), Some(Color::rgb8(255, 255, 255)));
    assert_eq!(
        Color::parse_hex("#f00c"),
        Some(Color::rgba8(255, 0, 0, 0xcc))
    );
    assert_eq!(Color::parse_hex("#1e1e2e"), Some(Color::hex(0x1e1e2e)));
    assert_eq!(
        Color::parse_hex("#1e1e2e80"),
        Some(Color::rgba8(0x1e, 0x1e, 0x2e, 0x80))
    );
    for text in ["1e1e2e", "#1e1e2", "#gggggg", ""] {
        assert_eq!(Color::parse_hex(text), None, "accepted {text:?}");
    }
}

#[test]
fn hex_round_trips() {
    for color in [
        Color::hex(0x1e1e2e),
        Color::WHITE,
        Color::BLACK,
        Color::rgba8(0x12, 0x34, 0x56, 0x78),
    ] {
        assert_eq!(Color::parse_hex(&color.to_hex()), Some(color));
    }
}

#[test]
fn srgb_transfer_matches_standard() {
    let cases: [(f64, f64); 5] = [
        (0.0, 0.0),
        (0.02, 0.02 / 12.92),
        (0.5, 0.214_041_140_482_232_55),
        (0.8, 0.603_827_338_855_338_8),
        (1.0, 1.0),
    ];
    for (encoded, linear) in cases {
        assert!(
            (srgb::to_linear_f64(encoded) - linear).abs() < 1e-12,
            "{encoded}"
        );
        assert!(
            (srgb::from_linear_f64(linear) - encoded).abs() < 1e-9,
            "{linear}"
        );
        assert!((f64::from(srgb::to_linear(encoded as f32)) - linear).abs() < 1e-6);
        assert!((f64::from(srgb::from_linear(linear as f32)) - encoded).abs() < 1e-5);
    }
    assert!((srgb::luminance([1.0, 1.0, 1.0]) - 1.0).abs() < 1e-6);
}
