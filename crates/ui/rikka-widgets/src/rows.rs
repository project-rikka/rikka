use rikka_text::Align as TextAlign;
use rikka_types::{Color, Point, Rect, Size};
use rikka_ui::text::TextRun;
use rikka_ui::{Action, EventCtx, PaintCtx, measure_text};

const TRAILING_SHARE: f32 = 0.3;

pub(crate) fn glyph_rect(x: f32, center_y: f32, size: f32) -> Rect {
    Rect::from_center(Point::new(x + size / 2.0, center_y), Size::new(size, size))
}

pub(crate) fn draw_cell(
    ctx: &mut PaintCtx<'_>,
    within: Rect,
    x: f32,
    width: f32,
    run: TextRun,
    color: Color,
) {
    let clip = Rect::new(x, within.y(), width.max(0.0), within.height());
    let run = run.ellipsized();
    ctx.clipped(clip, |ctx| ctx.draw_text_line(within, x, width, run, color));
}

pub(crate) fn draw_trailing(
    ctx: &mut PaintCtx<'_>,
    row: Rect,
    inset: f32,
    width: f32,
    run: TextRun,
    color: Color,
) -> f32 {
    let reserved = reserved(measure_text(&run, None).width, row.width(), width);
    draw_cell(
        ctx,
        row,
        row.max_x() - inset - reserved,
        reserved,
        run.align(TextAlign::Right),
        color,
    );
    (width - reserved - inset).max(0.0)
}

pub(crate) fn draw_tag(
    ctx: &mut PaintCtx<'_>,
    room: Rect,
    run: TextRun,
    color: Color,
) -> Option<f32> {
    let width = measure_text(&run, None).width.ceil();
    if width > room.width() {
        return None;
    }
    let x = room.max_x() - width;
    draw_cell(ctx, room, x, width, run, color);
    Some(x)
}

fn reserved(requested: f32, row_width: f32, width: f32) -> f32 {
    requested
        .min(row_width * TRAILING_SHARE)
        .min(width)
        .max(0.0)
}
pub(crate) fn room_for_trailing(requested: f32) -> f32 {
    requested / TRAILING_SHARE
}

pub(crate) fn select(
    ctx: &mut EventCtx<'_>,
    on_select: &Action<(usize,)>,
    current: Option<usize>,
    next: Option<usize>,
) {
    let Some(next) = next.filter(|next| Some(*next) != current) else {
        return;
    };
    ctx.act(on_select, (next,));
}
