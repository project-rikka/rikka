use rikka_types::{Color, Point, Rect, Size};
use rikka_ui::{IconGlyph, MaterialSymbol, PaintCtx, Quad};

const CIRCLE: f32 = 0.8;
const SQUARE: f32 = 0.72;
const BAR_WIDTH: f32 = 0.42;
const BAR_HEIGHT: f32 = 1.05;
const HOLLOW: f32 = 1.5;
const ROUNDING: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyShape {
    Diamond,
    Hourglass,
    Circle,
    Square,
    Bar,
    Ring,
    Outline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyGlyph {
    pub incoming: KeyShape,
    pub outgoing: KeyShape,
}

impl KeyGlyph {
    #[must_use]
    pub const fn whole(shape: KeyShape) -> KeyGlyph {
        KeyGlyph {
            incoming: shape,
            outgoing: shape,
        }
    }

    pub fn paint(self, ctx: &mut PaintCtx<'_>, center: Point, size: f32, color: Color) {
        let whole = Rect::from_center(center, Size::new(size, size));
        if self.incoming == self.outgoing {
            paint_shape(ctx, self.incoming, whole, color);
            return;
        }
        let left = Rect::new(whole.x(), whole.y(), size / 2.0, size);
        let right = Rect::new(center.x, whole.y(), size / 2.0, size);
        ctx.clipped(left, |ctx| paint_shape(ctx, self.incoming, whole, color));
        ctx.clipped(right, |ctx| paint_shape(ctx, self.outgoing, whole, color));
    }
}

fn paint_shape(ctx: &mut PaintCtx<'_>, shape: KeyShape, whole: Rect, color: Color) {
    let side = whole.width();
    let center = whole.center();
    let symbol = |symbol: MaterialSymbol| IconGlyph::from(symbol).spec(side).fill(1.0);
    let centred = |share: f32| Rect::from_center(center, Size::new(side * share, side * share));
    match shape {
        KeyShape::Diamond => ctx.draw_icon(whole, symbol(MaterialSymbol::DIAMOND), color),
        KeyShape::Hourglass => ctx.draw_icon(whole, symbol(MaterialSymbol::HOURGLASS), color),
        KeyShape::Circle => ctx.draw_quad(
            Quad::new(centred(CIRCLE))
                .fill(color)
                .radius(side * CIRCLE / 2.0),
        ),
        KeyShape::Ring => ctx.draw_quad(
            Quad::new(centred(CIRCLE))
                .stroke(HOLLOW, color)
                .radius(side * CIRCLE / 2.0),
        ),
        KeyShape::Square => {
            ctx.draw_quad(Quad::new(centred(SQUARE)).fill(color).radius(ROUNDING));
        }
        KeyShape::Outline => ctx.draw_quad(
            Quad::new(centred(SQUARE))
                .stroke(HOLLOW, color)
                .radius(ROUNDING),
        ),
        KeyShape::Bar => ctx.draw_quad(
            Quad::new(Rect::from_center(
                center,
                Size::new(side * BAR_WIDTH, side * BAR_HEIGHT),
            ))
            .fill(color)
            .radius(ROUNDING),
        ),
    }
}
