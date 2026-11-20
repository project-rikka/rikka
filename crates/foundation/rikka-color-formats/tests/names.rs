use rikka_color_formats::{ColorName, Rgb};

#[test]
fn names_by_lightness_saturation_hue() {
    for (rgb, named) in [
        (Rgb::new(255, 255, 255), ColorName::White),
        (Rgb::new(0, 0, 0), ColorName::Black),
        (Rgb::new(200, 200, 200), ColorName::LightGray),
        (Rgb::new(128, 128, 128), ColorName::Gray),
        (Rgb::new(60, 60, 60), ColorName::DarkGray),
        (Rgb::new(255, 0, 0), ColorName::Red),
        (Rgb::new(0, 0, 255), ColorName::Blue),
        (Rgb::new(0, 255, 0), ColorName::Green),
        (Rgb::new(0, 128, 0), ColorName::DarkGreen),
        (Rgb::new(128, 0, 0), ColorName::DarkRed),
        (Rgb::new(255, 255, 0), ColorName::Yellow),
    ] {
        assert_eq!(rgb.name(), named, "{rgb}");
    }
}
