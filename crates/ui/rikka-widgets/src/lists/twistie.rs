use std::f32::consts::FRAC_PI_2;

use rikka_types::{Color, Rect};
use rikka_ui::{Element, ImageSource, Layout, Length, PaintCtx, Sprite, Widget, motion};

const CHEVRON: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 -960 960 960"><path fill="#fff" d="M504-480 320-664l56-56 240 240-240 240-56-56 184-184Z"/></svg>"##;

const TURN: &str = "twistie";

thread_local! {
    static SHAPE: ImageSource = ImageSource::vector(CHEVRON);
}

pub fn twistie(open: bool, side: f32, color: Color) -> Element {
    Element::new(Twistie { open, side, color })
}

pub fn paint_twistie(ctx: &mut PaintCtx<'_>, rect: Rect, open: bool, color: Color) {
    let progress = ctx.animate_curve(TURN, if open { 1.0 } else { 0.0 }, motion::MORPH);
    let shape = SHAPE.with(Clone::clone);
    ctx.draw_sprite(
        Sprite::new(rect, shape)
            .turned(progress * FRAC_PI_2)
            .tinted(color),
    );
}

#[derive(Debug)]
pub struct Twistie {
    open: bool,
    side: f32,
    color: Color,
}

impl Widget for Twistie {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::px(self.side), Length::px(self.side))
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        paint_twistie(ctx, bounds, self.open, self.color);
    }

    fn same(&self, other: &Self) -> bool {
        self.open == other.open && self.side == other.side && self.color == other.color
    }
}
