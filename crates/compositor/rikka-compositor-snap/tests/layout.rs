use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_snap::{Edged, at_edge, templates};

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect<i32, Logical> {
    Rect::new((x, y).into(), (w, h).into())
}

#[test]
fn templates_follow_the_display_shape() {
    assert_eq!(templates(Size::from((1366, 728))).len(), 4);
    assert_eq!(templates(Size::from((1920, 1040))).len(), 6);
    let upright = templates(Size::from((1080, 1880)));
    assert_eq!(upright.len(), 4);
    let area = rect(0, 40, 1080, 1880);
    let stacked: Vec<_> = upright[0].zones(area).collect();
    assert_eq!(
        stacked,
        vec![rect(0, 40, 1080, 940), rect(0, 980, 1080, 940)]
    );
}

#[test]
fn zones_tile_the_area_exactly() {
    for area in [
        rect(0, 40, 1920, 1040),
        rect(1920, 0, 1365, 767),
        rect(-1080, 31, 1079, 1889),
    ] {
        for layout in templates(area.size) {
            let zones: Vec<_> = layout.zones(area).collect();
            let covered: i64 = zones
                .iter()
                .map(|zone| i64::from(zone.size.w) * i64::from(zone.size.h))
                .sum();
            assert_eq!(
                covered,
                i64::from(area.size.w) * i64::from(area.size.h),
                "{layout:?} in {area:?}"
            );
            for (index, zone) in zones.iter().enumerate() {
                assert!(area.contains_rect(*zone), "{zone:?} in {area:?}");
                assert_eq!(layout.zone(area, index), *zone);
                for other in &zones[index + 1..] {
                    assert!(!zone.overlaps(*other), "{zone:?} over {other:?}");
                }
            }
        }
    }
}

#[test]
fn edges_and_corners_pick_zones() {
    let screen = rect(0, 0, 1920, 1080);
    let area = rect(0, 40, 1920, 1040);
    let zone_at = |x: f64, y: f64| match at_edge(screen, area, Point::from((x, y)))? {
        Edged::Maximize => Some(area),
        Edged::Zone { layout, zone } => Some(layout.zone(area, zone)),
    };
    let cases = [
        ((960.0, 44.0), Some(area)),
        ((960.0, 60.0), None),
        ((2.0, 560.0), Some(rect(0, 40, 960, 1040))),
        ((1918.0, 560.0), Some(rect(960, 40, 960, 1040))),
        ((2.0, 100.0), Some(rect(0, 40, 960, 520))),
        ((1918.0, 100.0), Some(rect(960, 40, 960, 520))),
        ((2.0, 1070.0), Some(rect(0, 560, 960, 520))),
        ((1918.0, 1070.0), Some(rect(960, 560, 960, 520))),
        ((960.0, 560.0), None),
        ((-20.0, 560.0), None),
    ];
    for (at, zone) in cases {
        assert_eq!(zone_at(at.0, at.1), zone, "{at:?}");
    }
    assert!(matches!(
        at_edge(screen, area, Point::from((960.0, 44.0))),
        Some(Edged::Maximize)
    ));
    assert_eq!(
        at_edge(screen, rect(0, 0, 0, 0), Point::from((0.0, 0.0))),
        None
    );
}
