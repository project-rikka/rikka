use std::time::Instant;

use desk_layout::{Guide, Projection};
use rikka_theme::Theme;
use rikka_types::{Color, Point, Rect};
use rikka_ui::{PaintCtx, Quad, TextAlign, TextRun, Weight};

use crate::session::Marks;

const LINE: f32 = 1.0;
const TICK: f32 = 7.0;
const LABEL_TEXT: f32 = 11.0;
const LABEL_HEIGHT: f32 = 18.0;
const LABEL_PAD: f32 = 6.0;
const LABEL_GAP: f32 = 4.0;
const DIGIT: f32 = 0.62;
const LABEL_DROP: f32 = 2.0;

pub(super) fn draw(marks: &Marks, projection: Projection, theme: &Theme, ctx: &mut PaintCtx<'_>) {
    if marks.guides.is_empty() {
        return;
    }
    if let Some(until) = marks.fading {
        if Instant::now() >= until {
            return;
        }
        ctx.wake_at(until);
    }
    let palette = theme.palette();
    for guide in &marks.guides {
        match *guide {
            Guide::Vertical { x, top, bottom } => {
                let from = projection.point(Point::new(x, top));
                let to = projection.point(Point::new(x, bottom));
                line(
                    Rect::new(from.x - LINE / 2.0, from.y, LINE, to.y - from.y),
                    palette.accent,
                    ctx,
                );
            }
            Guide::Horizontal { y, left, right } => {
                let from = projection.point(Point::new(left, y));
                let to = projection.point(Point::new(right, y));
                line(
                    Rect::new(from.x, from.y - LINE / 2.0, to.x - from.x, LINE),
                    palette.accent,
                    ctx,
                );
            }
            Guide::Spacing { span, across_x } => {
                spacing(
                    projection.rect(span),
                    span,
                    across_x,
                    palette.danger,
                    palette.on_danger,
                    ctx,
                );
            }
        }
    }
}

fn line(rect: Rect, color: Color, ctx: &mut PaintCtx<'_>) {
    ctx.draw_quad(Quad::new(rect).fill(color));
}

fn spacing(
    shown: Rect,
    desk: Rect,
    across_x: bool,
    color: Color,
    ink: Color,
    ctx: &mut PaintCtx<'_>,
) {
    let centre = shown.center();
    let gap = match across_x {
        true => desk.width(),
        false => desk.height(),
    };
    match across_x {
        true => {
            line(
                Rect::new(shown.min_x(), centre.y - LINE / 2.0, shown.width(), LINE),
                color,
                ctx,
            );
            for x in [shown.min_x(), shown.max_x() - LINE] {
                line(Rect::new(x, centre.y - TICK / 2.0, LINE, TICK), color, ctx);
            }
        }
        false => {
            line(
                Rect::new(centre.x - LINE / 2.0, shown.min_y(), LINE, shown.height()),
                color,
                ctx,
            );
            for y in [shown.min_y(), shown.max_y() - LINE] {
                line(Rect::new(centre.x - TICK / 2.0, y, TICK, LINE), color, ctx);
            }
        }
    }
    let label = format!("{}", gap.round() as i32);
    let width = label.chars().count() as f32 * LABEL_TEXT * DIGIT + LABEL_PAD * 2.0;
    let pill = match across_x {
        true => Rect::new(
            centre.x - width / 2.0,
            centre.y - LABEL_GAP - LABEL_HEIGHT,
            width,
            LABEL_HEIGHT,
        ),
        false => Rect::new(
            centre.x + LABEL_GAP,
            centre.y - LABEL_HEIGHT / 2.0,
            width,
            LABEL_HEIGHT,
        ),
    };
    ctx.draw_quad(Quad::new(pill).radius(LABEL_HEIGHT / 2.0).fill(color));
    let run = TextRun::new(label, LABEL_TEXT)
        .no_wrap()
        .weight(Weight::SEMI_BOLD)
        .align(TextAlign::Center);
    let text_line = Rect::new(
        pill.min_x(),
        pill.center().y - LABEL_HEIGHT / 2.0 + LABEL_DROP,
        pill.width(),
        LABEL_HEIGHT,
    );
    ctx.draw_text(text_line, run, ink);
}
