use paint_document::Edge;
use rikka_app::prelude::*;

const DASH: f32 = 4.0;
const THICKNESS: f32 = 1.0;
const LIGHT: Color = Color::WHITE;
const DARK: Color = Color::BLACK;

pub(crate) fn draw(
    ctx: &mut PaintCtx<'_>,
    edges: &[Edge],
    bounds: Rect,
    place: impl Fn((u32, u32)) -> Point,
) {
    for edge in edges {
        let (from, to) = (place(edge.from), place(edge.to));
        let horizontal = edge.from.1 == edge.to.1;
        let (start, end, across, room, depth) = match horizontal {
            true => (
                from.x,
                to.x,
                from.y,
                bounds.min_x()..bounds.max_x(),
                bounds.min_y()..bounds.max_y(),
            ),
            false => (
                from.y,
                to.y,
                from.x,
                bounds.min_y()..bounds.max_y(),
                bounds.min_x()..bounds.max_x(),
            ),
        };
        if across + THICKNESS < depth.start || across > depth.end {
            continue;
        }
        let (start, end) = (start.max(room.start), end.min(room.end));
        if start >= end {
            continue;
        }
        let line = |from: f32, to: f32, color: Color| {
            let rect = match horizontal {
                true => Rect::new(from, across, to - from, THICKNESS),
                false => Rect::new(across, from, THICKNESS, to - from),
            };
            Quad::new(rect).fill(color)
        };
        ctx.draw_quad(line(start, end, LIGHT));
        let mut dash = (start / (DASH * 2.0)).floor() * DASH * 2.0;
        while dash < end {
            let (from, to) = (dash.max(start), (dash + DASH).min(end));
            if from < to {
                ctx.draw_quad(line(from, to, DARK));
            }
            dash += DASH * 2.0;
        }
    }
}
