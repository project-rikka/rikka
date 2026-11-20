use rikka_theme::Palette;
use rikka_types::{Rect, Size};
use rikka_ui::{IconSource, PaintCtx, Quad};
use shell_apps::AppIcons;

use crate::tasks::{BORDER, LIFTED};

const TILE_ROUNDING: f32 = 0.3;

const SYMBOL: f32 = 0.6;

const PICTURE: f32 = 0.72;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Art {
    icon: IconSource,
    scale: f32,
}

impl Art {
    #[must_use]
    pub(crate) fn resolve(icon: IconSource, icons: &AppIcons) -> Art {
        let scale = icons.footprint(&icon);
        Art { icon, scale }
    }

    pub(crate) fn paint(
        &self,
        ctx: &mut PaintCtx<'_>,
        at: Rect,
        zoom: f32,
        palette: &Palette,
        presence: f32,
    ) {
        let share = match self.icon {
            IconSource::Picture(_) => PICTURE * self.scale,
            IconSource::Glyph(_) | IconSource::Mask(_) => SYMBOL,
        };
        let side = at.width() * share;
        let crisp = side * ctx.scale() * LIFTED;
        let icon = self.rastered_at(Size::new(crisp, crisp));
        ctx.with_opacity(presence, |ctx| {
            ctx.zoomed(zoom, at.center(), |ctx| {
                ctx.draw_quad(
                    Quad::new(at)
                        .fill(palette.surface)
                        .stroke(BORDER, palette.outline)
                        .radius(at.width() * TILE_ROUNDING),
                );
                let inner = Rect::from_center(at.center(), Size::new(side, side));
                ctx.draw_icon_source(inner, &icon, side, palette.accent);
            });
        });
    }

    fn rastered_at(&self, pixels: Size) -> IconSource {
        match &self.icon {
            IconSource::Picture(picture) => {
                IconSource::Picture(picture.clone().rastered_for(pixels))
            }
            IconSource::Mask(picture) => IconSource::Mask(picture.clone().rastered_for(pixels)),
            IconSource::Glyph(glyph) => IconSource::Glyph(*glyph),
        }
    }
}
