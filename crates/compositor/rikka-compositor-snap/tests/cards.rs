use rikka_compositor_geometry::{Logical, Rect};
use rikka_compositor_snap::{fitted, shape_of, slots};

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect<i32, Logical> {
    Rect::new((x, y).into(), (w, h).into())
}

#[test]
fn cards_fit_the_zone_without_overlap() {
    let zone = rect(960, 40, 960, 1040);
    let aspect = 16.0 / 9.0;
    for count in 1..=12 {
        let slots = slots(zone, aspect, count);
        assert_eq!(slots.len(), count.min(9), "{count} windows");
        for (index, slot) in slots.iter().enumerate() {
            assert!(zone.contains_rect(slot.card), "{:?}", slot.card);
            assert!(slot.card.contains_rect(slot.picture));
            assert!(slot.card.contains_rect(slot.label));
            assert!(!slot.picture.overlaps(slot.label));
            assert_eq!(slot.label.size.w, slot.picture.size.w);
            let drawn = shape_of(slot.picture.size);
            assert!((drawn - aspect).abs() < 0.05, "picture is {drawn}");
            for other in &slots[index + 1..] {
                assert!(!slot.card.overlaps(other.card), "{slot:?} over {other:?}");
            }
        }
    }
    assert!(slots(zone, aspect, 0).is_empty());
    assert!(slots(rect(0, 0, 60, 60), aspect, 3).is_empty());
}

#[test]
fn few_cards_stay_on_one_row_while_wide_enough() {
    let zone = rect(0, 40, 1920, 1040);
    let pair = slots(zone, 16.0 / 9.0, 2);
    assert_eq!(pair[0].card.loc.y, pair[1].card.loc.y);
    assert!(pair[0].picture.size.w >= 240);
    let many = slots(rect(960, 40, 960, 1040), 16.0 / 9.0, 6);
    assert!(many[0].card.loc.y < many[5].card.loc.y);
}

#[test]
fn fitted_keeps_the_shape_inside_the_room() {
    let room = rect(100, 100, 400, 300);
    let wide = fitted(room, 2.0);
    assert_eq!(
        (wide.loc.x, wide.loc.y, wide.size.w, wide.size.h),
        (100.0, 150.0, 400.0, 200.0)
    );
    let tall = fitted(room, 0.5);
    assert_eq!(
        (tall.loc.x, tall.loc.y, tall.size.w, tall.size.h),
        (225.0, 100.0, 150.0, 300.0)
    );
    assert_eq!(fitted(room, f64::NAN), room.to_f64());
    assert_eq!(shape_of(rect(0, 0, 300, 0).size), 0.0);
}
