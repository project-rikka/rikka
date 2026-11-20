use rikka_text::Affinity;
use rikka_types::{Color, Point, Rect, Size};
use rikka_ui::{PaintCtx, Quad, TextDocument, palette};

use rikka_figure::{Grips, TURN_GAP, Typeset};

const LINE: f32 = 1.0;
const HANDLE: f32 = 8.0;
const KNOB: f32 = 9.0;
const KNOB_CORE: f32 = 5.0;
const BAND_TINT: f32 = 0.12;
const SELECTED_TINT: f32 = 0.35;
const CARET: f32 = 1.5;
const UNDERLINE: f32 = 1.5;
const UPRIGHT: f32 = 0.01;

pub fn paint_typing(
    ctx: &mut PaintCtx<'_>,
    typeset: &Typeset,
    document: &TextDocument,
    shown: &dyn Fn(Point) -> Option<Point>,
) -> Option<Rect> {
    let accent = palette().accent;
    let content = document.content();
    let caret_at = content.caret();
    let preedit = content.preedit().len();
    let affinity = match preedit {
        0 => content.affinity(),
        _ => Affinity::After,
    };
    let selected = content.selection();
    let within = match content.preedit_cursor() {
        Some((start, _)) if preedit > 0 => start,
        _ => preedit,
    };
    drop(content);
    let corners = |rect: Rect| -> Option<[Point; 4]> {
        let [a, b, c, d] = rect.corners();
        Some([shown(a)?, shown(b)?, shown(c)?, shown(d)?])
    };
    let shifted = |at: usize| match at > caret_at {
        true => at + preedit,
        false => at,
    };
    for band in typeset.selection(shifted(selected.start)..shifted(selected.end)) {
        if let Some(placed) = corners(band) {
            match upright(&placed) {
                Some(rect) => {
                    ctx.draw_quad(Quad::new(rect).fill(accent.scale_alpha(SELECTED_TINT)))
                }
                None => paint_loop(ctx, &placed, accent.scale_alpha(SELECTED_TINT * 2.0)),
            }
        }
    }
    for band in typeset.selection(caret_at..caret_at + preedit) {
        let Some([_, _, right, left]) = corners(band) else {
            continue;
        };
        paint_rule(ctx, left, right, UNDERLINE, accent, true);
    }
    let caret = typeset.caret(caret_at + within, affinity);
    let top = shown(Point::new(caret.x(), caret.y()))?;
    let bottom = shown(Point::new(caret.x(), caret.max_y()))?;
    Some(paint_rule(ctx, top, bottom, CARET, accent, false))
}

fn paint_rule(
    ctx: &mut PaintCtx<'_>,
    from: Point,
    to: Point,
    thickness: f32,
    color: Color,
    above: bool,
) -> Rect {
    let (low, high) = (from.min(to), from.max(to));
    let rect = match (
        (high.x - low.x).abs() < UPRIGHT,
        (high.y - low.y).abs() < UPRIGHT,
    ) {
        (true, _) => Rect::new(low.x - thickness / 2.0, low.y, thickness, high.y - low.y),
        (false, true) if above => Rect::new(low.x, low.y - thickness, high.x - low.x, thickness),
        (false, true) => Rect::new(low.x, low.y, high.x - low.x, thickness),
        (false, false) => {
            paint_line(ctx, from, to, color);
            return Rect::new(low.x, low.y, (high.x - low.x).max(LINE), high.y - low.y);
        }
    };
    ctx.draw_quad(Quad::new(rect).fill(color));
    rect
}

fn upright(corners: &[Point; 4]) -> Option<Rect> {
    let level = |from: Point, to: Point| (from.y - to.y).abs() < UPRIGHT;
    let plumb = |from: Point, to: Point| (from.x - to.x).abs() < UPRIGHT;
    let square = level(corners[0], corners[1])
        && plumb(corners[1], corners[2])
        && level(corners[2], corners[3])
        && plumb(corners[3], corners[0]);
    square.then(|| {
        let (low, high) = (corners[0].min(corners[2]), corners[0].max(corners[2]));
        Rect::new(low.x, low.y, high.x - low.x, high.y - low.y)
    })
}

pub fn paint_grips(
    ctx: &mut PaintCtx<'_>,
    grips: &Grips,
    shown: &dyn Fn(Point) -> Point,
    scale: f32,
) {
    let accent = palette().accent;
    match grips {
        Grips::Ends(from, to) => paint_line(ctx, shown(*from), shown(*to), accent),
        Grips::Frame { frame, .. } => {
            let corners = frame.corners().map(shown);
            paint_loop(ctx, &corners, accent);
        }
    }
    if let Some((top, turn)) = grips.turn(TURN_GAP / scale) {
        paint_line(ctx, shown(top), shown(turn), accent);
        ctx.draw_circle(shown(turn), KNOB, accent);
        ctx.draw_circle(shown(turn), KNOB_CORE, palette().surface);
    }
    for (_, spot) in grips.handles() {
        ctx.draw_quad(
            Quad::new(Rect::from_center(shown(spot), Size::new(HANDLE, HANDLE)))
                .fill(palette().surface)
                .stroke(LINE, accent),
        );
    }
}

pub fn paint_band(ctx: &mut PaintCtx<'_>, from: Point, to: Point) {
    let accent = palette().accent;
    let (low, high) = (from.min(to), from.max(to));
    ctx.draw_quad(
        Quad::new(Rect::new(low.x, low.y, high.x - low.x, high.y - low.y))
            .fill(accent.scale_alpha(BAND_TINT))
            .stroke(LINE, accent),
    );
}

pub fn paint_loop(ctx: &mut PaintCtx<'_>, corners: &[Point], color: Color) {
    match corners {
        [from, to] => paint_line(ctx, *from, *to, color),
        _ => {
            for (from, to) in corners.iter().zip(corners.iter().cycle().skip(1)) {
                paint_line(ctx, *from, *to, color);
            }
        }
    }
}
pub fn paint_dashed(
    ctx: &mut PaintCtx<'_>,
    points: &[Point],
    closed: bool,
    pattern: &[(f32, Color)],
) {
    if pattern.is_empty() || pattern.iter().any(|(length, _)| *length <= 0.0) {
        return;
    }
    let closing = points.first().copied().filter(|_| closed);
    let mut dash = 0;
    let mut left = pattern[0].0;
    for (from, to) in points
        .iter()
        .copied()
        .zip(points.iter().copied().skip(1).chain(closing))
    {
        let length = from.distance(to);
        let along = |distance: f32| from + (to - from) * (distance / length);
        let mut at = 0.0;
        while at < length {
            let step = left.min(length - at);
            let color = pattern[dash].1;
            if color.a > 0.0 {
                paint_line(ctx, along(at), along(at + step), color);
            }
            at += step;
            left -= step;
            if left <= 0.0 {
                dash = (dash + 1) % pattern.len();
                left = pattern[dash].0;
            }
        }
    }
}

pub fn paint_line(ctx: &mut PaintCtx<'_>, from: Point, to: Point, color: Color) {
    let bounds = ctx.bounds();
    let (across, down) = (to.x - from.x, to.y - from.y);
    if across.abs() < 0.5 || down.abs() < 0.5 {
        let (low, high) = (from.min(to), from.max(to));
        let rect = Rect::new(
            low.x - LINE / 2.0,
            low.y - LINE / 2.0,
            (high.x - low.x).max(LINE),
            (high.y - low.y).max(LINE),
        );
        if let Some(shown) = rect.intersection(&bounds) {
            ctx.draw_quad(Quad::new(shown).fill(color));
        }
        return;
    }
    let Some((from, to)) = clipped(from, to, bounds.inflate(LINE)) else {
        return;
    };
    ctx.draw_segment(from, to, LINE * 1.5, color);
}

fn clipped(from: Point, to: Point, within: Rect) -> Option<(Point, Point)> {
    let along = to - from;
    let (mut enter, mut leave) = (0.0f32, 1.0f32);
    for (step, room) in [
        (-along.x, from.x - within.min_x()),
        (along.x, within.max_x() - from.x),
        (-along.y, from.y - within.min_y()),
        (along.y, within.max_y() - from.y),
    ] {
        if step == 0.0 {
            if room < 0.0 {
                return None;
            }
            continue;
        }
        let at = room / step;
        match step < 0.0 {
            true => enter = enter.max(at),
            false => leave = leave.min(at),
        }
    }
    (enter <= leave).then(|| (from + along * enter, from + along * leave))
}
