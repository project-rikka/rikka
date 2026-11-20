use rikka_compositor_geometry::{Logical, Rect};

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect<i32, Logical> {
    Rect::new((x, y).into(), (w, h).into())
}

#[test]
fn contains_excludes_far_edges() {
    let area = rect(10, 20, 30, 40);
    assert!(area.contains((10, 20)));
    assert!(area.contains((39, 59)));
    assert!(!area.contains((40, 20)));
    assert!(!area.contains((10, 60)));
    assert!(area.contains_rect(area));
    assert!(!area.contains_rect(rect(10, 20, 30, 45)));
    assert!(!area.contains_rect(rect(41, 61, 30, 40)));
}

#[test]
fn subtract_many_stays_disjoint() {
    let rects = Rect::subtract_rects_many_in_place(
        vec![rect(0, 0, 10, 10), rect(20, 0, 10, 10)],
        [rect(5, 0, 20, 5), rect(0, 5, 30, 5)],
    );
    let area: i32 = rects.iter().map(|r| r.size.w * r.size.h).sum();
    assert_eq!(area, 25 + 25);
    for (index, a) in rects.iter().enumerate() {
        for b in &rects[index + 1..] {
            assert!(!a.overlaps(*b));
        }
    }
    assert_eq!(
        rect(0, 0, 10, 10).subtract_rects([rect(0, 0, 5, 10), rect(5, 0, 5, 10)]),
        []
    );
    assert_eq!(
        Rect::subtract_rects_many_in_place(Vec::new(), [rect(0, 0, 1, 1)]),
        []
    );
}
