use animate_cast::{LARGEST_SIDE, Resolution, SMALLEST_SIDE, parse_side};
use rikka_types::Rect;

const PIXEL: f32 = 1e-3;

fn whole(value: f32) -> bool {
    (value - value.round()).abs() < PIXEL
}

#[test]
fn frame_letterboxes_on_whole_device_pixels() {
    let cases = [
        (
            Resolution::FULL_HD,
            Rect::new(0.0, 0.0, 1000.0, 1000.0),
            1.0,
        ),
        (
            Resolution::FULL_HD,
            Rect::new(10.5, 20.25, 801.0, 300.0),
            1.25,
        ),
        (
            Resolution::FULL_HD.turned(),
            Rect::new(0.0, 0.0, 1280.0, 720.0),
            2.0,
        ),
        (
            Resolution::new(1080, 1080).expect("a square fits"),
            Rect::new(3.0, 7.0, 640.0, 333.0),
            1.5,
        ),
    ];
    for (resolution, bounds, scale) in cases {
        let frame = resolution.frame_in(bounds, scale);
        let device = [frame.x(), frame.y(), frame.width(), frame.height()].map(|v| v * scale);
        assert!(
            device.iter().all(|value| whole(*value)),
            "{frame:?} at {scale}"
        );
        assert!(frame.min_x() >= bounds.min_x() - 1.0 / scale);
        assert!(frame.max_x() <= bounds.max_x() + 1.0 / scale);
        assert!(frame.min_y() >= bounds.min_y() - 1.0 / scale);
        assert!(frame.max_y() <= bounds.max_y() + 1.0 / scale);
        let fills = (frame.width() - bounds.width()).abs() <= 1.0 / scale
            || (frame.height() - bounds.height()).abs() <= 1.0 / scale;
        assert!(fills, "{frame:?} touches neither side of {bounds:?}");
        let aspect = resolution.width() as f32 / resolution.height() as f32;
        assert!((frame.width() - frame.height() * aspect).abs() <= 1.0 / scale + PIXEL);
        let left = frame.min_x() - bounds.min_x();
        let right = bounds.max_x() - frame.max_x();
        assert!(
            (left - right).abs() <= 1.0 / scale + PIXEL,
            "centred across"
        );
    }
}

#[test]
fn typed_sides_stay_in_range() {
    let cases = [
        ("1920", Some(1920)),
        (" 640 ", Some(640)),
        ("15", None),
        ("8193", None),
        ("wide", None),
        ("", None),
    ];
    for (typed, side) in cases {
        assert_eq!(parse_side(typed), side, "{typed:?}");
    }
    assert_eq!(parse_side(&SMALLEST_SIDE.to_string()), Some(SMALLEST_SIDE));
    assert_eq!(parse_side(&LARGEST_SIDE.to_string()), Some(LARGEST_SIDE));
}
