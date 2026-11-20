use rikka_app::prelude::*;

const DOT: f32 = 1.5;
const DOT_STEP: f32 = 4.0;
const STROKE_STEP: f32 = 0.7;

pub fn stroke(ctx: &mut PaintCtx<'_>, from: Point, to: Point, width: f32, tint: Color) {
    let step = (width * STROKE_STEP).max(0.5);
    let steps = (from.distance(to) / step).ceil().max(1.0) as usize;
    for at in 0..=steps {
        let along = at as f32 / steps as f32;
        ctx.draw_circle(from + (to - from) * along, width, tint);
    }
}

pub fn disc(at: Point, radius: f32) -> Quad {
    Quad::new(Rect::from_circle(at, radius)).radius(radius)
}

pub fn dotted(ctx: &mut PaintCtx<'_>, from: Point, to: Point, tint: Color) {
    let length = (to.x - from.x).hypot(to.y - from.y);
    let steps = (length / DOT_STEP).floor() as usize;
    for step in 1..steps {
        let along = step as f32 / steps as f32;
        let at = Point::new(
            from.x + (to.x - from.x) * along,
            from.y + (to.y - from.y) * along,
        );
        ctx.draw_circle(at, DOT, tint);
    }
}

pub fn centred(ctx: &mut PaintCtx<'_>, rect: Rect, run: TextRun, line: f32, tint: Color) {
    let run = run.no_wrap();
    let width = measure_text(&run, None).width;
    ctx.draw_text(
        Rect::new(
            rect.center().x - width / 2.0,
            rect.center().y - line / 2.0,
            width + 2.0,
            line,
        ),
        run,
        tint,
    );
}

pub const LEFT_HAND_TINT: Color = Color::rgb(0.25, 0.7, 1.0);
pub const RIGHT_HAND_TINT: Color = Color::rgb(1.0, 0.35, 0.45);
pub const PIN_TINT: Color = Color::rgb(1.0, 0.45, 0.2);
