use rikka_mesh_bevel::{
    DragValue, PIXEL_MARGIN, ValueDrag, WidthType, next_width_type, pan_segments,
    segments_from_value,
};

#[test]
fn drag_starts_at_current_value() {
    let kinds = [
        (DragValue::Width, 0.01, 0.35),
        (DragValue::WidthPercent, 100.0 / 96.0, 40.0),
        (DragValue::ProfileShape, 1.0 / 96.0, 0.7),
        (DragValue::Segments, 4.0 / 96.0, 3.0),
    ];
    for (kind, scale, current) in kinds {
        let drag = ValueDrag::new(kind, scale, 120.0, current);
        let value = drag.value(120.0, None, false);
        assert!((value - current).abs() < 1.0e-4, "{kind:?}: {value}");
    }
}

#[test]
fn drag_maps_distance_through_scale() {
    let drag = ValueDrag::new(DragValue::Width, 0.01, 50.0, 0.0);
    let value = drag.value(50.0 + PIXEL_MARGIN + 30.0, None, false);
    assert!((value - 0.3).abs() < 1.0e-5);
    let snapped = drag.value(50.0 + PIXEL_MARGIN + 34.0, None, true);
    assert!((snapped - 0.3).abs() < 1.0e-5);
    let precise = drag.value(50.0 + PIXEL_MARGIN + 30.0, Some(0.1), false);
    assert!((precise - 0.12).abs() < 1.0e-5);
}

#[test]
fn drag_values_clamp_to_their_ranges() {
    let percent = ValueDrag::new(DragValue::WidthPercent, 1.0, 0.0, 0.0);
    assert_eq!(percent.value(1000.0, None, false), 100.0);
    let width = ValueDrag::new(DragValue::Width, 1.0, 100.0, 0.0);
    assert_eq!(width.value(0.0, None, false), 0.0);
    assert_eq!(segments_from_value(0.2), 1);
    assert_eq!(segments_from_value(2.6), 3);
}

#[test]
fn segment_panning_stops_at_one() {
    assert_eq!(pan_segments(1.2, -50.0), 1.0);
    assert!((pan_segments(2.0, 25.0) - 2.5).abs() < 1.0e-6);
}

#[test]
fn width_type_cycle_skips_absolute() {
    let mut width_type = WidthType::Offset;
    let mut seen = Vec::new();
    for _ in 0..4 {
        width_type = next_width_type(width_type);
        seen.push(width_type);
    }
    assert_eq!(
        seen,
        [
            WidthType::Width,
            WidthType::Depth,
            WidthType::Percent,
            WidthType::Offset
        ]
    );
    assert_eq!(next_width_type(WidthType::Absolute), WidthType::Offset);
}
