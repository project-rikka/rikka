use rikka_types::{RoundedRect, Size};

struct Band {
    y: i32,
    rows: i32,
    left: i32,
    right: i32,
}

pub fn rectangles(pane: RoundedRect, into: &mut Vec<[i32; 4]>) {
    let left = pane.rect.min_x().round() as i32;
    let top = pane.rect.min_y().round() as i32;
    let right = pane.rect.max_x().round() as i32;
    let bottom = pane.rect.max_y().round() as i32;
    let (width, height) = (right - left, bottom - top);
    if width <= 0 || height <= 0 {
        return;
    }

    let radii = pane
        .radii
        .clamp_for_size(Size::new(width as f32, height as f32));
    let top_zone = zone(radii.top_left, radii.top_right, height);
    let bottom_zone = zone(radii.bottom_left, radii.bottom_right, height - top_zone);

    let mut bands: Vec<Band> = Vec::with_capacity((top_zone + bottom_zone + 1) as usize);
    let mut push = |y: i32, rows: i32, left: i32, right: i32| match bands.last_mut() {
        Some(last) if last.left == left && last.right == right && last.y + last.rows == y => {
            last.rows += rows;
        }
        _ => bands.push(Band {
            y,
            rows,
            left,
            right,
        }),
    };

    for row in 0..top_zone {
        let above = row as f32;
        push(
            top + row,
            1,
            cut(radii.top_left, above),
            cut(radii.top_right, above),
        );
    }
    let straight = height - top_zone - bottom_zone;
    if straight > 0 {
        push(top + top_zone, straight, 0, 0);
    }
    for row in 0..bottom_zone {
        let y = bottom - bottom_zone + row;
        let below = (bottom - y - 1) as f32;
        push(
            y,
            1,
            cut(radii.bottom_left, below),
            cut(radii.bottom_right, below),
        );
    }

    into.extend(bands.into_iter().filter_map(|band| {
        let span = width - band.left - band.right;
        (span > 0).then_some([left + band.left, band.y, span, band.rows])
    }));
}

fn zone(one: f32, other: f32, room: i32) -> i32 {
    (one.max(other).max(0.0).ceil() as i32).min(room.max(0))
}

fn cut(radius: f32, distance: f32) -> i32 {
    if radius <= 0.0 || distance >= radius {
        return 0;
    }
    let from_centre = radius - distance;
    let across = (radius * radius - from_centre * from_centre)
        .max(0.0)
        .sqrt();
    (radius - across).ceil() as i32
}
