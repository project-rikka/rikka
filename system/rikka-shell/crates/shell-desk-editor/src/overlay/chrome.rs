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
    let edge = shape.edge();
    let insets = shape.insets();
    let desktop = RoundedRect::new(
        Rect::new(
            card.min_x() + insets.left() * scale,
            card.min_y() + insets.top() * scale,
            (card.width() - (insets.left() + insets.right()) * scale).max(0.0),
            (card.height() - (insets.top() + insets.bottom()) * scale).max(0.0),
        ),
        CornerRadii::uniform(shape.corner() * scale),
    );
    ctx.draw_quad(Quad::new(card).fill(palette.background).cut_out(desktop));

    let length = edge.length(card);
    let middle = shape.reserved() * scale / 2.0;
    let at = |along: f32| edge.place(card, Rect::new(along, middle, 0.0, 0.0)).origin;
    let island = edge.place(
        card,
        Rect::from_center(
            Point::new(length / 2.0, middle),
            Size::new(ISLAND.width * scale, ISLAND.height * scale),
        ),
    );
    ctx.draw_quad(
        Quad::new(island)
            .radius(island.width().min(island.height()) / 2.0)
            .fill(palette.text.with_alpha(FAINT)),
    );
    let logo = (SIDE + LOGO / 2.0) * scale;
    ctx.draw_circle(at(logo), LOGO * scale, palette.accent);
    for mark in 0..MARKS {
        let along = logo + (LOGO / 2.0 + SIDE / 2.0 + MARK_PITCH * mark as f32) * scale;
        let tint = match mark {
            0 => palette.text,
            _ => palette.text.with_alpha(DIM),
        };
        ctx.draw_circle(at(along), MARK * scale, tint);
    }
    for status in 0..STATUS {
        let along = length - (SIDE + STATUS_DOT / 2.0 + STATUS_PITCH * status as f32) * scale;
        ctx.draw_circle(at(along), STATUS_DOT * scale, palette.text.with_alpha(DIM));
    }
}
