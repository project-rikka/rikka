use std::path::Path;
use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_ui::{DragIcon, Element};

const ART: f32 = 72.0;
const MARGIN: f32 = 14.0;
const SIDE: f32 = ART + MARGIN * 2.0;
const INSET: f32 = 8.0;
const GLYPH: f32 = 48.0;
const STACKED: f32 = 5.0;
const BADGE: f32 = 22.0;
const BADGE_TEXT: f32 = 12.0;

#[derive(Clone, Debug)]
pub(crate) enum Art {
    Picture { path: Arc<Path>, pixels: Size },
    Glyph { symbol: MaterialSymbol, tint: Color },
}

pub(crate) fn lifted(theme: Theme, art: Art, count: usize) -> DragIcon {
    let hotspot = Point::new(SIDE / 2.0, MARGIN + ART * 0.6);
    DragIcon::new(Size::new(SIDE, SIDE), hotspot, theme, move || {
        Element::new(Lifted {
            art: art.clone(),
            count,
        })
        .width(Length::FILL)
        .height(Length::FILL)
    })
}

struct Lifted {
    art: Art,
    count: usize,
}

impl Lifted {
    fn paint_stack(&self, ctx: &mut PaintCtx<'_>, front: Rect) {
        let palette = ctx.palette();
        let behind = self.count.saturating_sub(1).min(2);
        for depth in (1..=behind).rev() {
            let offset = STACKED * depth as f32;
            let card = Rect::new(
                front.x() + offset,
                front.y() + offset,
                front.width(),
                front.height(),
            );
            ctx.draw_quad(
                Quad::new(card)
                    .fill(palette.surface_variant)
                    .radius(Corner::Panel)
                    .stroke(1.0, palette.outline),
            );
        }
        ctx.draw_quad(
            Quad::new(front)
                .fill(palette.surface)
                .radius(Corner::Panel)
                .stroke(1.0, palette.outline),
        );
    }

    fn paint_art(&self, ctx: &mut PaintCtx<'_>, front: Rect) {
        let inner = front.inflate(-INSET);
        match &self.art {
            Art::Picture { path, pixels } => {
                ctx.draw_image(ImageFit::Contain.place(inner, *pixels), Arc::clone(path));
            }
            Art::Glyph { symbol, tint } => {
                let glyph = Rect::from_center(inner.center(), Size::new(GLYPH, GLYPH));
                ctx.draw_icon(glyph, IconGlyph::from(*symbol).spec(GLYPH), *tint);
            }
        }
    }

    fn paint_badge(&self, ctx: &mut PaintCtx<'_>, front: Rect) {
        if self.count < 2 {
            return;
        }
        let palette = ctx.palette();
        let counted = self.count.to_string();
        let run = TextRun::new(counted.as_str(), BADGE_TEXT).no_wrap();
        let written = measure_text(&run, None);
        let width = (written.width + 12.0).max(BADGE);
        let badge = Rect::new(
            front.max_x() - width / 2.0 - 4.0,
            front.y() - BADGE / 2.0 + 4.0,
            width,
            BADGE,
        );
        ctx.draw_quad(Quad::new(badge).fill(palette.accent).radius(BADGE / 2.0));
        let text = Rect::from_center(
            badge.center(),
            Size::new(written.width + 2.0, written.height),
        );
        ctx.draw_text(text, run, palette.on_accent);
    }
}

impl Widget for Lifted {
    type State = ();

    fn paint(&self, _: &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let front = Rect::new(bounds.x() + MARGIN, bounds.y() + MARGIN, ART, ART);
        self.paint_stack(ctx, front);
        self.paint_art(ctx, front);
        self.paint_badge(ctx, front);
    }
}
