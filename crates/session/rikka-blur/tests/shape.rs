use rikka_blur::rectangles;
use rikka_types::{CornerRadii, Rect, RoundedRect};

fn rects(pane: RoundedRect) -> Vec<[i32; 4]> {
    let mut rects = Vec::new();
    rectangles(pane, &mut rects);
    rects
}

fn inside(pane: RoundedRect, x: f32, y: f32) -> bool {
    let rect = pane.rect;
    if x < rect.min_x() || x > rect.max_x() || y < rect.min_y() || y > rect.max_y() {
        return false;
    }
    let radii = pane.radii.clamp_for_size(rect.size);
    let corners = [
        (rect.min_x(), rect.min_y(), radii.top_left, 1.0, 1.0),
        (rect.max_x(), rect.min_y(), radii.top_right, -1.0, 1.0),
        (rect.max_x(), rect.max_y(), radii.bottom_right, -1.0, -1.0),
        (rect.min_x(), rect.max_y(), radii.bottom_left, 1.0, -1.0),
    ];
    for (cx, cy, radius, sx, sy) in corners {
        let (centre_x, centre_y) = (cx + sx * radius, cy + sy * radius);
        let outside_x = (x - centre_x) * sx < 0.0;
        let outside_y = (y - centre_y) * sy < 0.0;
        if outside_x && outside_y {
            let (dx, dy) = (x - centre_x, y - centre_y);
            if dx * dx + dy * dy > radius * radius {
                return false;
            }
        }
    }
    true
}

#[test]
fn square_pane_is_one_rectangle() {
    let rects = rects(RoundedRect::rect(Rect::new(10.0, 20.0, 100.0, 40.0)));
    assert_eq!(rects, vec![[10, 20, 100, 40]]);
}

#[test]
fn empty_pane_has_no_rectangles() {
    assert!(rects(RoundedRect::rect(Rect::new(0.0, 0.0, 0.0, 40.0))).is_empty());
    assert!(rects(RoundedRect::rect(Rect::new(0.0, 0.0, 40.0, 0.0))).is_empty());
}

#[test]
fn staircase_stays_inside_rounding() {
    for radius in [0.0, 1.0, 6.0, 12.0, 24.0, 999.0] {
        let pane = RoundedRect::new(
            Rect::new(5.0, 7.0, 220.0, 130.0),
            CornerRadii::new(radius, radius * 0.5, radius, 0.0),
        );
        for rect in rects(pane) {
            let [x, y, w, h] = rect.map(|value| value as f32);
            for (px, py) in [
                (x, y),
                (x + w, y),
                (x, y + h),
                (x + w, y + h),
                (x + w * 0.5, y + h * 0.5),
            ] {
                assert!(
                    inside(pane, px, py),
                    "radius {radius}: {rect:?} reaches ({px}, {py})",
                );
            }
        }
    }
}

#[test]
fn oversized_radius_stays_inside_pane() {
    let pane = RoundedRect::new(Rect::new(0.0, 0.0, 20.0, 20.0), CornerRadii::uniform(40.0));
    let rects = rects(pane);
    assert!(!rects.is_empty());
    for [x, y, w, h] in rects {
        assert!(x >= 0 && y >= 0 && x + w <= 20 && y + h <= 20);
    }
}
