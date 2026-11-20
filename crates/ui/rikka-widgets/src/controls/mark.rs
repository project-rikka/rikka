use rikka_symbols::MaterialSymbol;
use rikka_text::IconSpec;
use rikka_theme::Theme;
use rikka_types::{Color, Rect};
use rikka_ui::{PaintCtx, Quad};

use crate::controls::checkbox::CheckState;

const BORDER: f32 = 1.5;

const RADIUS_RATIO: f32 = 0.3;
const MARKED_RADIUS: f32 = 0.42;

const GLYPH_RATIO: f32 = 0.72;

pub(crate) struct Mark {
    pub(crate) marked: bool,
    pub(crate) fill: Color,
    pub(crate) radius: f32,
    pub(crate) border: Color,
    pub(crate) glyph: Option<MaterialSymbol>,
    pub(crate) grown: f32,
    pub(crate) ink: Color,
}

impl Mark {
    pub(crate) fn settled(state: CheckState, theme: &Theme, edge: f32, enabled: bool) -> Mark {
        let palette = theme.palette();
        let marked = state.is_marked();
        Mark {
            marked,
            fill: match marked {
                true => theme.dim(palette.accent, !enabled),
                false => Color::TRANSPARENT,
            },
            radius: edge * if marked { MARKED_RADIUS } else { RADIUS_RATIO },
            border: theme.dim(palette.text_muted, !enabled),
            glyph: state.glyph(),
            grown: if marked { 1.0 } else { 0.0 },
            ink: theme.dim(palette.on_accent, !enabled),
        }
    }

    pub(crate) fn paint(&self, ctx: &mut PaintCtx<'_>, box_rect: Rect) {
        let mut quad = Quad::new(box_rect).fill(self.fill).radius(self.radius);
        if !self.marked {
            quad = quad.stroke(BORDER, self.border);
        }
        ctx.draw_quad(quad);

        let Some(symbol) = self.glyph.filter(|_| self.grown > 0.0) else {
            return;
        };
        let glyph = (box_rect.width() * GLYPH_RATIO * self.grown).round();
        ctx.draw_icon(
            box_rect,
            IconSpec::new(symbol.codepoint(), glyph),
            self.ink.scale_alpha(self.grown.min(1.0)),
        );
    }
}

impl std::fmt::Debug for Mark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mark")
            .field("marked", &self.marked)
            .field("glyph", &self.glyph)
            .finish_non_exhaustive()
    }
}
