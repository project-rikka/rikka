use rikka_types::RoundedRect;

use crate::shape;

pub(crate) type Area = [i32; 4];

pub fn opaque_rectangles(opaque: &[RoundedRect], glass: &[RoundedRect], into: &mut Vec<[i32; 4]>) {
    let start = into.len();
    for shape in opaque {
        interior(*shape, into);
    }
    let mut panes = Vec::new();
    for pane in glass {
        shape::rectangles(*pane, &mut panes);
    }
    let mut tail = into.split_off(start);
    for pane in panes {
        tail = tail
            .into_iter()
            .flat_map(|area| subtract(area, pane))
            .collect();
    }
    into.extend(tail);
}

fn interior(shape: RoundedRect, into: &mut Vec<Area>) {
    let rect = shape.rect;
    let left = rect.min_x().ceil() as i32;
    let top = rect.min_y().ceil() as i32;
    let right = rect.max_x().floor() as i32;
    let bottom = rect.max_y().floor() as i32;
    if right <= left || bottom <= top {
        return;
    }
    let radii = shape.radii.clamp_for_size(rect.size);
    let reach = |radius: f32| (radius * RoundedRect::CORNER_REACH).ceil() as i32;
    let (top_left, top_right) = (reach(radii.top_left), reach(radii.top_right));
    let (bottom_left, bottom_right) = (reach(radii.bottom_left), reach(radii.bottom_right));
    let across = [
        left,
        top + top_left.max(top_right),
        right,
        bottom - bottom_left.max(bottom_right),
    ];
    let down = [
        left + top_left.max(bottom_left),
        top,
        right - top_right.max(bottom_right),
        bottom,
    ];
    for [x0, y0, x1, y1] in [across, down] {
        if x1 > x0 && y1 > y0 {
            into.push([x0, y0, x1 - x0, y1 - y0]);
        }
    }
}

fn subtract(area: Area, cut: Area) -> Vec<Area> {
    let [x, y, width, height] = area;
    let (right, bottom) = (x + width, y + height);
    let [cx, cy, cw, ch] = cut;
    let (cut_right, cut_bottom) = (cx + cw, cy + ch);
    if cx >= right || cut_right <= x || cy >= bottom || cut_bottom <= y {
        return vec![area];
    }
    let (inner_top, inner_bottom) = (cy.max(y), cut_bottom.min(bottom));
    [
        [x, y, width, inner_top - y],
        [x, inner_bottom, width, bottom - inner_bottom],
        [x, inner_top, cx.max(x) - x, inner_bottom - inner_top],
        [
            cut_right.min(right),
            inner_top,
            right - cut_right.min(right),
            inner_bottom - inner_top,
        ],
    ]
    .into_iter()
    .filter(|[_, _, width, height]| *width > 0 && *height > 0)
    .collect()
}
