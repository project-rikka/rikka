use rikka_compositor_geometry::{Damage, Logical, Rect};

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect<i32, Logical> {
    Rect::new((x, y).into(), (w, h).into())
}

#[test]
fn adjacent_damage_merges_when_cheap() {
    let mut damage = Damage::new();
    damage.add(rect(0, 0, 10, 10));
    damage.add(rect(10, 0, 10, 10));
    damage.add(rect(50, 50, 5, 5));
    damage.add(rect(5, 5, 10, 10));
    assert_eq!(
        damage.iter().collect::<Vec<_>>(),
        [rect(50, 50, 5, 5), rect(0, 0, 20, 15)]
    );
}

#[test]
fn scattered_rects_collapse_to_bounds() {
    let mut damage = Damage::new();
    for index in 0..32 {
        damage.add(rect(index * 10, 0, 5, 5));
    }
    assert_eq!(damage.iter().len(), 32);
    damage.add(rect(0, 100, 5, 5));
    assert_eq!(damage.iter().collect::<Vec<_>>(), [rect(0, 0, 315, 105)]);
}
