use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_snap::{Bar, Hit, Reveal};

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect<i32, Logical> {
    Rect::new((x, y).into(), (w, h).into())
}

fn bar() -> Bar {
    Bar::measure(rect(0, 0, 1920, 1080), rect(0, 40, 1920, 1040)).expect("the bar fits")
}

#[test]
fn bar_sits_centred_below_the_top_of_the_usable_area() {
    let bar = bar();
    assert_eq!(bar.tiles().len(), 6);
    let open = bar.shown(1.0);
    assert_eq!((open.loc.x, open.loc.y), (458.0, 48.0));
    assert_eq!((open.size.w, open.size.h), (1004.0, 108.0));
    assert_eq!(bar.shown(0.0).size.h, 12.0);
    for tile in bar.tiles() {
        assert!(open.contains_rect(tile.rect.to_f64()));
        for zone in tile.zones() {
            assert!(tile.rect.contains_rect(zone), "{zone:?} in {:?}", tile.rect);
        }
    }
    assert!(Bar::measure(rect(0, 0, 400, 300), rect(0, 0, 400, 300)).is_none());
    assert!(Bar::measure(rect(0, 0, 1920, 1080), rect(0, 0, 0, 0)).is_none());
}

#[test]
fn pointer_over_a_tile_hits_its_zone() {
    let bar = bar();
    for (index, tile) in bar.tiles().iter().enumerate() {
        for (zone, shown) in tile.zones().enumerate() {
            let centre = shown.loc.to_f64()
                + Point::new(f64::from(shown.size.w) / 2.0, f64::from(shown.size.h) / 2.0);
            assert_eq!(bar.hit(centre), Some(Hit { tile: index, zone }));
        }
    }
    assert_eq!(bar.hit(Point::from((20.0, 500.0))), None);
}

#[test]
fn bar_peeks_opens_and_hides() {
    let bar = bar();
    let far = Point::from((960.0, 700.0));
    let near = Point::from((960.0, 70.0));
    let inside = Point::from((960.0, 120.0));
    let just_below = Point::from((960.0, 180.0));

    assert_eq!(bar.revealed(far, Reveal::Hidden), Reveal::Hidden);
    assert_eq!(bar.revealed(near, Reveal::Hidden), Reveal::Peek);
    assert_eq!(bar.revealed(inside, Reveal::Hidden), Reveal::Hidden);
    assert_eq!(bar.revealed(inside, Reveal::Peek), Reveal::Open);
    assert_eq!(bar.revealed(just_below, Reveal::Open), Reveal::Open);
    assert_eq!(bar.revealed(just_below, Reveal::Peek), Reveal::Hidden);
    assert_eq!(bar.revealed(far, Reveal::Open), Reveal::Hidden);
    assert_eq!(bar.revealed(near, Reveal::Peek), Reveal::Open);
}
