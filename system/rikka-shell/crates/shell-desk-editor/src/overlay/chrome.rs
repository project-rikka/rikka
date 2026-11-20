use desk_layout::Projection;
use rikka_theme::Theme;
use rikka_types::{CornerRadii, Point, Rect, RoundedRect, Size};
use rikka_ui::{PaintCtx, Quad};
use shell_kit::taskbar::Shape;

const ISLAND: Size = Size::new(196.0, 28.0);
const LOGO: f32 = 18.0;
const SIDE: f32 = 22.0;
const MARK: f32 = 7.0;
const MARK_PITCH: f32 = 14.0;
const MARKS: usize = 3;
const STATUS: usize = 4;
const STATUS_PITCH: f32 = 22.0;
const STATUS_DOT: f32 = 12.0;
const FAINT: f32 = 0.16;
const DIM: f32 = 0.5;

pub(super) fn taskbar(
    card: Rect,
    projection: Projection,
    shape: Shape,
    theme: &Theme,
    ctx: &mut PaintCtx<'_>,
) {
    let palette = theme.palette();
    let scale = projection.scale();
    let depth = shape.reserved() * scale;
    let corner = shape.corner() * scale;
    let band = Rect::new(card.min_x(), card.min_y(), card.width(), depth + corner);
    let below = RoundedRect::new(
        Rect::new(
            card.min_x(),
            card.min_y() + depth,
            card.width(),
            corner * 2.0,
        ),
        CornerRadii::new(corner, corner, 0.0, 0.0),
    );
    ctx.draw_quad(Quad::new(band).fill(palette.background).cut_out(below));

    let middle = card.min_y() + depth / 2.0;
    let island = Size::new(ISLAND.width * scale, ISLAND.height * scale);
    ctx.draw_quad(
        Quad::new(Rect::from_center(
            Point::new(card.center().x, middle),
            island,
        ))
        .radius(island.height / 2.0)
        .fill(palette.text.with_alpha(FAINT)),
    );
    let logo = Point::new(card.min_x() + (SIDE + LOGO / 2.0) * scale, middle);
    ctx.draw_circle(logo, LOGO * scale, palette.accent);
    for at in 0..MARKS {
        let x = logo.x + (LOGO / 2.0 + SIDE / 2.0 + MARK_PITCH * at as f32) * scale;
        let tint = match at {
            0 => palette.text,
            _ => palette.text.with_alpha(DIM),
        };
        ctx.draw_circle(Point::new(x, middle), MARK * scale, tint);
    }
    for at in 0..STATUS {
        let x = card.max_x() - (SIDE + STATUS_DOT / 2.0 + STATUS_PITCH * at as f32) * scale;
        ctx.draw_circle(
            Point::new(x, middle),
            STATUS_DOT * scale,
            palette.text.with_alpha(DIM),
        );
    }
}
