use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_scrolling::{Frame, Spacing, Strip};

pub(crate) fn frame() -> Frame {
    let screen = Rect::new(Point::new(0, 0), Size::new(1000, 800));
    Frame {
        work: screen,
        full: screen,
        spacing: Spacing {
            inner: 10,
            outer: 10,
        },
    }
}

pub(crate) fn strip_with(windows: &[u32]) -> Strip<u32> {
    let mut strip = Strip::new(frame());
    for window in windows {
        let focused = strip.active();
        strip.insert(*window, focused.as_ref());
        strip.settle();
    }
    strip
}

pub(crate) fn laid(strip: &mut Strip<u32>) -> Vec<(u32, [i32; 4])> {
    strip.settle();
    strip
        .tiles()
        .into_iter()
        .map(|(window, rect)| (window, corners(rect)))
        .collect()
}

pub(crate) fn corners(rect: Rect<i32, Logical>) -> [i32; 4] {
    [rect.loc.x, rect.loc.y, rect.size.w, rect.size.h]
}
