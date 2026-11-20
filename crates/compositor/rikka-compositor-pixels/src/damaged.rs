use rikka_compositor_geometry::{Buffer, Rect, Size};

const MAX_RECTS: usize = 16;

pub fn damaged_rects(
    damage: impl IntoIterator<Item = Rect<i32, Buffer>>,
    size: Size<i32, Buffer>,
    into: &mut Vec<Rect<i32, Buffer>>,
) {
    into.clear();
    let whole = Rect::from_size(size);
    for rect in damage {
        let Some(mut rect) = rect.intersection(whole) else {
            continue;
        };
        while let Some(at) = into.iter().position(|other| other.overlaps(rect)) {
            rect = rect.merge(into.swap_remove(at));
        }
        into.push(rect);
    }
    let Some(bounds) = into
        .iter()
        .copied()
        .reduce(|bounds, rect| bounds.merge(rect))
    else {
        return;
    };
    let area = |rect: &Rect<i32, Buffer>| i64::from(rect.size.w) * i64::from(rect.size.h);
    let covered: i64 = into.iter().map(area).sum();
    if into.len() > MAX_RECTS || covered * 4 >= area(&bounds) * 3 {
        into.clear();
        into.push(bounds);
    }
}
