use desk_layout::{Area, MARGIN, Size, Spot};
use rikka_types::Rect;

#[test]
fn rect_round_trips_through_spot() {
    let area = Area::for_desk(Rect::new(0.0, 40.0, 1920.0, 1040.0));
    let room = area.room();
    for (x, y) in [(0.0, 0.0), (137.0, 402.0), (1000.0, 11.0)] {
        let extent = Size::Medium.extent();
        let rect = Rect::new(
            room.min_x() + x,
            room.min_y() + y,
            extent.width,
            extent.height,
        );
        let spot = area.spot_of(rect);
        assert_eq!(area.place(Size::Medium, spot), rect, "{x},{y}");
    }
}

#[test]
fn spots_scale_to_other_display() {
    let small = Area::for_desk(Rect::new(0.0, 0.0, 1280.0, 720.0));
    let large = Area::for_desk(Rect::new(0.0, 0.0, 2560.0, 1440.0));
    let size = Size::Small;
    let extent = size.extent();

    let corner = Spot::new(1.0, 0.0);
    let placed = large.place(size, corner);
    assert_eq!(placed.max_x(), 2560.0 - MARGIN);
    assert_eq!(placed.min_y(), MARGIN);

    let centred = small.place(size, Spot::CENTER);
    assert_eq!(centred.center().x, 640.0);
    let centred = large.place(size, Spot::CENTER);
    assert_eq!(centred.min_x(), ((2560.0 - extent.width) / 2.0).round());
}

#[test]
fn spots_clamp_to_area() {
    let area = Area::for_desk(Rect::new(0.0, 0.0, 800.0, 600.0));
    for spot in [Spot::new(f32::NAN, 2.0), Spot::new(-1.0, f32::INFINITY)] {
        let rect = area.place(Size::Small, spot);
        assert!(area.room().contains_rect(&rect), "{spot:?} gave {rect:?}");
    }
    let outside = Rect::new(-400.0, 900.0, 100.0, 100.0);
    assert!(area.room().contains_rect(&area.contain(outside)));
}
