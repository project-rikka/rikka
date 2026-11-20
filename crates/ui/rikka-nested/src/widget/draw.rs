use std::f32::consts::FRAC_PI_2;

use rikka_types::{Point, Rect};
use rikka_ui::{Layout, PaintCtx, Quad, Sprite, Style, Widget};

use crate::frame::{Layer, LayerContent};

pub(crate) fn draw_layer(ctx: &mut PaintCtx<'_>, origin: Point, layer: &Layer) {
    let rect = layer.rect.translate(origin);
    if layer.alpha < 1.0 {
        let alpha = layer.alpha.max(0.0);
        ctx.with_opacity(alpha, |ctx| draw_content(ctx, rect, layer));
    } else {
        draw_content(ctx, rect, layer);
    }
}

fn draw_content(ctx: &mut PaintCtx<'_>, rect: Rect, layer: &Layer) {
    let source = match &layer.content {
        LayerContent::Solid(color) => {
            ctx.draw_quad(Quad::new(rect).fill(*color));
            return;
        }
        content => match content.source() {
            Some(source) => source,
            None => return,
        },
    };
    let transform = layer.transform;
    let sprite = |within: Rect| {
        Sprite::new(within, source.clone())
            .turned(f32::from(transform.quarter_turns) * FRAC_PI_2)
            .mirrored(transform.flipped, false)
    };
    let cropped = layer
        .crop
        .filter(|crop| crop.size.width > 0.0 && crop.size.height > 0.0)
        .filter(|_| transform.quarter_turns == 0 && !transform.flipped);
    match cropped {
        Some(crop) => {
            let across = rect.size.width / crop.size.width;
            let down = rect.size.height / crop.size.height;
            let whole = Rect::new(
                rect.origin.x - crop.origin.x * across,
                rect.origin.y - crop.origin.y * down,
                layer.buffer.width * across,
                layer.buffer.height * down,
            );
            ctx.clipped(rect, |ctx| ctx.draw_sprite(sprite(whole)));
        }
        None => ctx.draw_sprite(sprite(rect)),
    }
}

#[derive(rikka_ui::Widget)]
pub(crate) struct LayerImage {
    layer: Layer,
    #[style]
    style: Style,
}

impl LayerImage {
    pub(crate) fn new(layer: Layer) -> Self {
        Self {
            layer,
            style: Style::default(),
        }
    }
}

impl Widget for LayerImage {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let mut layer = self.layer.clone();
        layer.rect = Rect::new(0.0, 0.0, bounds.size.width, bounds.size.height);
        draw_layer(ctx, bounds.origin, &layer);
    }
}
