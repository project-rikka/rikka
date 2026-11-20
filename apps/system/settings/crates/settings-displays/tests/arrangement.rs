use settings_displays::Bounds;
use settings_displays::arrangement::snap;

fn bounds(x: i32, y: i32, width: i32, height: i32) -> Bounds {
    Bounds {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn snaps_flush_to_nearby_display() {
    let snapped = snap(bounds(1970, 40, 2560, 1440), &[bounds(0, 0, 1920, 1080)]);
    assert_eq!(snapped, (1920, 0));
}

#[test]
fn pushes_overlapping_display_clear() {
    let (x, y) = snap(bounds(1700, 900, 1920, 1080), &[bounds(0, 0, 1920, 1080)]);
    let clear = x >= 1920 || x <= -1920 || y >= 1080 || y <= -1080;
    assert!(clear, "({x}, {y}) still covers the other display");
}
