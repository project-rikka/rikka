use desk_layout::{Size, WidgetId};
use rikka_theme::{Corner, Theme};
use rikka_types::{Rect, Size as Extent};
use rikka_ui::{IconGlyph, MaterialSymbol, PaintCtx, Quad, TextAlign, TextRun, motion};

use crate::session::Tool;
use crate::sizes;

const CHIP: f32 = 30.0;
const PAD: f32 = 4.0;
pub(crate) const GAP: f32 = 2.0;
const DIVIDE: f32 = 9.0;
const DIVIDER_INSET: f32 = 7.0;
const LIFT: f32 = 12.0;
const BORDER: f32 = 1.0;
const TEXT: f32 = 12.0;
const LINE: f32 = 16.0;
const GLYPH: f32 = 17.0;
const DANGER_ALPHA: f32 = 0.14;
const RADIUS: f32 = Corner::Panel.radius();
const CHIP_RADIUS: f32 = Corner::Panel.nested(PAD);

pub(super) struct Bar {
    pub(super) widget: WidgetId,
    pub(super) sizes: &'static [Size],
    pub(super) size: Size,
    pub(super) hovered: Option<Tool>,
}

impl Bar {
    pub(super) fn draw(
        &self,
        ring: Rect,
        card: Rect,
        others: &[Rect],
        theme: &Theme,
        ctx: &mut PaintCtx<'_>,
    ) -> Vec<(Tool, Rect)> {
        let palette = theme.palette();
        let bar = self.place(ring, card, others);
        ctx.draw_pane(
            Quad::new(bar)
                .radius(RADIUS)
                .fill(palette.surface)
                .stroke(BORDER, palette.outline),
        );

        let chip = |index: f32| {
            Rect::new(
                bar.min_x() + PAD + index * (CHIP + GAP),
                bar.min_y() + PAD,
                CHIP,
                CHIP,
            )
        };
        let selected_index = self
            .sizes
            .iter()
            .position(|size| *size == self.size)
            .map(|index| index as f32);
        let seat = selected_index
            .map(|index| ctx.animate_curve(("seat", self.widget.0), index, motion::INDICATOR));

        let mut tools = Vec::with_capacity(self.sizes.len() + 1);
        for (index, size) in self.sizes.iter().enumerate() {
            let rect = chip(index as f32);
            if self.hovered == Some(Tool::Size(*size)) && Some(index as f32) != selected_index {
                ctx.draw_quad(
                    Quad::new(rect)
                        .radius(CHIP_RADIUS)
                        .fill(theme.hover_layer()),
                );
            }
            tools.push((Tool::Size(*size), rect));
        }
        if let Some(seat) = seat {
            ctx.draw_quad(
                Quad::new(chip(seat))
                    .radius(CHIP_RADIUS)
                    .fill(palette.selection),
            );
        }
        for (index, size) in self.sizes.iter().enumerate() {
            let rect = chip(index as f32);
            let emphasis = seat.map_or(0.0, |seat| (1.0 - (seat - index as f32).abs()).max(0.0));
            let ink = palette.text_muted.mix(palette.on_selection, emphasis);
            let label = sizes::short(*size);
            let run = TextRun::new(label.as_str(), TEXT)
                .no_wrap()
                .medium()
                .align(TextAlign::Center);
            let line = Rect::new(rect.min_x(), rect.center().y - LINE / 2.0, CHIP, LINE);
            ctx.draw_text(line, run, ink);
        }

        let divider = chip(self.sizes.len() as f32).min_x() - GAP + DIVIDE / 2.0;
        ctx.draw_quad(
            Quad::new(Rect::new(
                divider - BORDER / 2.0,
                bar.min_y() + PAD + DIVIDER_INSET,
                BORDER,
                CHIP - DIVIDER_INSET * 2.0,
            ))
            .fill(palette.outline),
        );

        let remove = Rect::new(bar.max_x() - PAD - CHIP, bar.min_y() + PAD, CHIP, CHIP);
        let ink = match self.hovered == Some(Tool::Remove) {
            true => {
                ctx.draw_quad(
                    Quad::new(remove)
                        .radius(CHIP_RADIUS)
                        .fill(palette.danger.with_alpha(DANGER_ALPHA)),
                );
                palette.danger
            }
            false => palette.text_muted,
        };
        ctx.draw_icon(
            Rect::from_center(remove.center(), Extent::new(GLYPH, GLYPH)),
            IconGlyph::from(MaterialSymbol::DELETE).spec(GLYPH),
            ink,
        );
        tools.push((Tool::Remove, remove));
        tools
    }

    fn place(&self, ring: Rect, card: Rect, others: &[Rect]) -> Rect {
        let count = self.sizes.len() as f32;
        let width = PAD * 2.0 + count * CHIP + (count - 1.0).max(0.0) * GAP + DIVIDE + CHIP;
        let height = PAD * 2.0 + CHIP;
        let x = (ring.center().x - width / 2.0)
            .min(card.max_x() - width - PAD)
            .max(card.min_x() + PAD);
        let above = Rect::new(x, ring.min_y() - LIFT - height, width, height);
        let below = Rect::new(x, ring.max_y() + LIFT, width, height);
        let fits = |bar: &Rect| {
            card.contains_rect(bar) && !others.iter().any(|other| other.intersects(bar))
        };
        match (fits(&above), fits(&below)) {
            (false, true) => below,
            (false, false) if above.min_y() < card.min_y() => below,
            _ => above,
        }
    }
}
