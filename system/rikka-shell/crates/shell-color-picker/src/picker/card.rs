use std::sync::Arc;

use rikka_color_formats::Rgb;
use rikka_theme::Corner;
use rikka_types::{Rect, RoundedRect};
use rikka_ui::text::{TextLayout, TextRun};
use rikka_ui::{PaintCtx, Quad};

const GAP: f32 = 10.0;
const EDGE: f32 = 8.0;
const PADDING: f32 = 10.0;
const SWATCH: f32 = 30.0;
const SPACING: f32 = 10.0;
const HAIR: f32 = 1.0;
const CAPTION: f32 = 0.85;

struct Line {
    run: TextRun,
    laid: Arc<TextLayout>,
}

impl Line {
    fn laid(run: TextRun) -> Line {
        let laid = rikka_text::layout_approx(run.as_shape(), None);
        Line { run, laid }
    }
}

pub(crate) struct Card {
    pub(crate) rect: Rect,
    rgb: Rgb,
    label: Line,
    named: Option<Line>,
}

impl Card {
    #[must_use]
    pub(crate) fn beside(
        ctx: &PaintCtx<'_>,
        projection: Rect,
        rgb: Rgb,
        value: String,
        name: Option<String>,
    ) -> Card {
        let theme = ctx.theme();
        let size = theme.typography().body_size;
        let label = Line::laid(TextRun::new(value, size).medium().no_wrap());
        let named =
            name.map(|name| Line::laid(TextRun::new(name, (size * CAPTION).round()).no_wrap()));

        let lines = [Some(&label), named.as_ref()].into_iter().flatten();
        let words = lines
            .clone()
            .map(|line| line.laid.widest())
            .fold(0.0, f32::max);
        let tall: f32 = lines.map(|line| line.laid.height()).sum();
        let width = PADDING * 2.0 + SWATCH + SPACING + words;
        let height = PADDING * 2.0 + SWATCH.max(tall);

        let room = ctx.bounds();
        let x = (projection.center().x - width / 2.0)
            .min(room.max_x() - EDGE - width)
            .max(room.min_x() + EDGE);
        let below = projection.max_y() + GAP;
        let y = match below + height > room.max_y() - EDGE {
            true => projection.min_y() - GAP - height,
            false => below,
        };
        let (left, top) = (ctx.snap(x), ctx.snap(y));
        let rect = Rect::new(
            left,
            top,
            ctx.snap(left + width) - left,
            ctx.snap(top + height) - top,
        );
        Card {
            rect,
            rgb,
            label,
            named,
        }
    }

    pub(crate) fn paint(self, ctx: &mut PaintCtx<'_>) {
        let palette = ctx.theme().palette();
        let card = self.rect;
        ctx.draw_quad(
            Quad::new(card)
                .fill(palette.surface)
                .stroke(HAIR, palette.outline)
                .radius(Corner::Panel),
        );
        ctx.glass(RoundedRect::new(card, Corner::Panel));

        let swatch = Rect::new(
            card.min_x() + PADDING,
            card.center().y - SWATCH / 2.0,
            SWATCH,
            SWATCH,
        );
        ctx.draw_quad(
            Quad::new(swatch)
                .fill(self.rgb.color())
                .stroke(HAIR, palette.outline)
                .radius(Corner::Control),
        );

        let left = swatch.max_x() + SPACING;
        let width = card.max_x() - PADDING - left;
        let tall =
            self.label.laid.height() + self.named.as_ref().map_or(0.0, |line| line.laid.height());
        let mut top = card.center().y - tall / 2.0;
        let lines = [
            (Some(self.label), palette.text),
            (self.named, palette.text_muted),
        ];
        for (line, ink) in lines {
            let Some(Line { run, laid }) = line else {
                continue;
            };
            let height = laid.height();
            ctx.draw_laid_text(Rect::new(left, top, width, height), run, laid, ink);
            top += height;
        }
    }
}
