use rikka_settings::Value;
use rikka_settings::session::color_picker::{Activation, Format};
use rikka_settings::session::displays::{self, Mode, Position};
use rikka_settings::session::{
    animation, appearance, capture, clipboard, color_picker, emoji, ime, language, network,
    notifications, pointer, power, remote, replay, translate, weather, windows,
};
use rikka_types::Transform;

#[test]
fn desktop_domains_are_consistent() {
    for (schema, named) in [
        (animation::schema(), animation::DOMAIN),
        (appearance::schema(), appearance::DOMAIN),
        (capture::schema(), capture::DOMAIN),
        (clipboard::schema(), clipboard::DOMAIN),
        (color_picker::schema(), color_picker::DOMAIN),
        (emoji::schema::<Activation>(), emoji::DOMAIN),
        (ime::schema(), ime::DOMAIN),
        (language::schema(), language::DOMAIN),
        (network::schema(), network::DOMAIN),
        (notifications::schema(), notifications::DOMAIN),
        (pointer::schema(), pointer::DOMAIN),
        (power::schema(), power::DOMAIN),
        (remote::schema(), remote::DOMAIN),
        (replay::schema(), replay::DOMAIN),
        (
            translate::schema::<Activation>(["high".to_owned()]),
            translate::DOMAIN,
        ),
        (weather::schema(), weather::DOMAIN),
        (windows::schema(), windows::DOMAIN),
    ] {
        assert_eq!(
            schema.validate().expect("domain should be valid").as_str(),
            named
        );
    }
}

#[test]
fn color_formats_round_trip_and_reject_invalid() {
    let schema = color_picker::schema();
    let formats = schema
        .key_named(color_picker::FORMATS)
        .expect("formats key should exist");
    assert_eq!(
        color_picker::formats(formats.default_value()),
        color_picker::built_in_formats()
    );

    let custom = [Format::new("Flutter", "Color(0xFF%ReX%GrX%BlX)")];
    let encoded = color_picker::formats_value(&custom);
    assert_eq!(color_picker::formats(&encoded), custom);
    assert!(formats.accepts(encoded).is_ok());

    for refused in [
        vec![Format::new("Twin", "%Re"), Format::new("Twin", "%Gr")],
        vec![Format::new("Twin", "%Re"), Format::new("Twin ", "%Gr")],
        vec![Format::new(" ", "%Re")],
    ] {
        assert!(
            formats
                .accepts(color_picker::formats_value(&refused))
                .is_err()
        );
    }
    assert!(formats.accepts(Value::String("HEX".to_owned())).is_err());
}

#[test]
fn display_values_round_trip_their_text() {
    for (text, width, height, refresh) in [
        ("3840x2160@59.997", 3840, 2160, 59_997),
        ("2560x1440@143.912", 2560, 1440, 143_912),
        ("1920x1080@60.000", 1920, 1080, 60_000),
    ] {
        let mode = Mode::parse(text).expect("the session's own spelling");
        assert_eq!(
            mode,
            Mode {
                width,
                height,
                refresh
            }
        );
        assert_eq!(mode.to_string(), text);
    }
    let modes = Mode::parse_all("3840x2160@59.997;nonsense;1920x1080@-60;1920x1080@60.000");
    assert_eq!(displays::listed(modes), "3840x2160@59.997;1920x1080@60.000");

    let position = Position { x: -1920, y: 40 };
    assert_eq!(Position::parse(&position.to_string()), Some(position));
    assert_eq!(Position::parse("12"), None);

    for transform in Transform::ALL {
        let name = displays::transform_name(transform);
        assert_eq!(displays::transform_named(name), Some(transform));
    }
    assert_eq!(
        displays::transform_named("flipped-90"),
        Some(Transform::Flipped90)
    );
    assert_eq!(displays::transform_named("sideways"), None);
}
