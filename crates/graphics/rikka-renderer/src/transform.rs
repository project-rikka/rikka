use rikka_types::{Rect, Transform};

pub(crate) fn turned_extent(transform: Transform, extent: (u32, u32)) -> (u32, u32) {
    match transform.swaps_axes() {
        true => (extent.1, extent.0),
        false => extent,
    }
}

pub(crate) fn unit_map(transform: Transform) -> ([f32; 4], [f32; 2]) {
    let axes = transform.axis_map();
    let along = |mirrored: bool| if mirrored { -1.0 } else { 1.0 };
    let (x, y) = (along(axes.mirrored_x), along(axes.mirrored_y));
    let linear = match axes.swapped {
        true => [0.0, x, y, 0.0],
        false => [x, 0.0, 0.0, y],
    };
    let offset = [
        f32::from(u8::from(axes.mirrored_x)),
        f32::from(u8::from(axes.mirrored_y)),
    ];
    (linear, offset)
}

pub(crate) fn placement(transform: Transform) -> [f32; 2] {
    let axes = transform.axis_map();
    [
        f32::from(u8::from(axes.swapped)),
        f32::from(u8::from(axes.mirrored_x) | (u8::from(axes.mirrored_y) << 1)),
    ]
}

pub(crate) fn sample_span(transform: Transform, region: Rect) -> ([f32; 2], [f32; 2], bool) {
    let (m, b) = unit_map(transform);
    let swapped = transform.swaps_axes();
    let along = if swapped { [m[1], m[2]] } else { [m[0], m[3]] };
    let (w, h) = (region.width(), region.height());
    let from = [region.min_x() + b[0] * w, region.min_y() + b[1] * h];
    let to = [from[0] + along[0] * w, from[1] + along[1] * h];
    (from, to, swapped)
}
