use rikka_types::{Color, Point, Rect, RoundedRect};
use rikka_ui::{PaintCtx, Quad};

use crate::picker::frame::Frame;

const DIAMETER: f32 = 156.0;

const RIM: f32 = 2.0;
const HAIR: f32 = 1.0;
const SHADE: Color = Color::rgba(0.0, 0.0, 0.0, 0.55);
const SHADOW: Color = Color::rgba(0.0, 0.0, 0.0, 0.35);
const SHADOW_BLUR: f32 = 10.0;
const GRID: Color = Color::rgba(0.5, 0.5, 0.5, 0.35);
const GRID_FROM: f32 = 7.0;
const OUTSIDE: Color = Color::rgba(0.08, 0.08, 0.08, 1.0);

pub(super) struct Loupe {
    pub(crate) projection: Rect,
    pixel: (u32, u32),
    left: f32,
    top: f32,
    cell: f32,
}

impl Loupe {
    #[must_use]
    pub(crate) fn aimed(ctx: &PaintCtx<'_>, pixel: (u32, u32), pointer: Point, cell: f32) -> Loupe {
        let scale = ctx.scale();
        let cell = (cell * scale).round().max(1.0) / scale;
        let left = ctx.snap(pointer.x - cell / 2.0);
        let top = ctx.snap(pointer.y - cell / 2.0);
        let centre = Point::new(left + cell / 2.0, top + cell / 2.0);
        Loupe {
            projection: Rect::from_circle(centre, DIAMETER / 2.0),
            pixel,
            left,
            top,
            cell,
        }
    }

    pub(crate) fn paint(&self, ctx: &mut PaintCtx<'_>, frame: &Frame) {
        let projection = self.projection;
        let radius = DIAMETER / 2.0;
        ctx.draw_quad(
            Quad::new(projection)
                .fill(SHADOW)
                .radius(radius)
                .blur(SHADOW_BLUR),
        );
        ctx.clipped(RoundedRect::new(projection, radius), |ctx| {
            ctx.draw_quad(Quad::new(projection).fill(OUTSIDE));
            let (x, y) = self.pixel;
            let picture = Rect::new(
                self.left - x as f32 * self.cell,
                self.top - y as f32 * self.cell,
                frame.width as f32 * self.cell,
                frame.height as f32 * self.cell,
            );
            ctx.draw_image_pixelated(picture, frame.picture.clone());
            if self.cell >= GRID_FROM {
                self.grid(ctx);
            }
            let marked = Rect::new(self.left, self.top, self.cell, self.cell);
            ctx.draw_quad(Quad::new(marked.inflate(HAIR)).stroke(HAIR, SHADE));
            ctx.draw_quad(Quad::new(marked).stroke(HAIR, Color::WHITE));
        });
        ctx.draw_quad(
            Quad::new(projection.inflate(RIM))
                .radius(radius + RIM)
                .stroke(HAIR, SHADE),
        );
        ctx.draw_quad(
            Quad::new(projection)
                .radius(radius)
                .stroke(RIM, Color::WHITE),
        );
    }

    fn grid(&self, ctx: &mut PaintCtx<'_>) {
        let (projection, cell) = (self.projection, self.cell);
        let mut x = self.left - ((self.left - projection.min_x()) / cell).ceil() * cell;
        while x <= projection.max_x() {
            ctx.draw_quad(
                Quad::new(Rect::new(x, projection.min_y(), HAIR, projection.height())).fill(GRID),
            );
            x += cell;
        }
        let mut y = self.top - ((self.top - projection.min_y()) / cell).ceil() * cell;
        while y <= projection.max_y() {
            ctx.draw_quad(
                Quad::new(Rect::new(projection.min_x(), y, projection.width(), HAIR)).fill(GRID),
            );
            y += cell;
        }
    }
}
