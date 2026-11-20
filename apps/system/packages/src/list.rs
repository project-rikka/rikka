use rikka_ui::{Styled, TextRun, measure_text, palette};
use std::sync::Arc;

use rikka_app::prelude::{
    Behavior, Element, Event, EventCtx, Modifiers, PaintCtx, PointerButton, Quad, RoundedRect,
    Widget, lazy_column,
};
use rikka_theme::Palette;
use rikka_types::{Color, Rect};
use rikka_ui::IconGlyph;
use rikka_widgets::Collection;

use crate::listing::{Line, Listing};
use crate::manager::Manager;

const ROW: f32 = 54.0;
const PAD: f32 = 14.0;
const GAP: f32 = 10.0;
const ICON: f32 = 20.0;
const TRAILING: f32 = 120.0;
const NAME: f32 = 13.5;
const SECONDARY: f32 = 12.0;
const HOVERED: f32 = 0.05;
const RULE: f32 = 1.0;

pub(crate) struct Packages {
    listing: Arc<Listing>,
    selected: Option<usize>,
}

impl Packages {
    pub(crate) fn new(listing: Listing, selected: Option<usize>) -> Self {
        Self {
            listing: Arc::new(listing),
            selected,
        }
    }

    pub(crate) fn view(self) -> Collection {
        let count = self.listing.len();
        let selected = self.selected;
        let listing = self.listing;
        lazy_column(count, move |slot| {
            Element::new(Row {
                listing: Arc::clone(&listing),
                index: slot.index,
                selected: selected == Some(slot.index),
            })
            .fill_width()
            .height(ROW)
        })
        .uniform(ROW)
        .focus(selected)
        .on_focus(|manager: &mut Manager, index: usize, _: Modifiers| manager.select(index))
    }
}

struct Row {
    listing: Arc<Listing>,
    index: usize,
    selected: bool,
}

impl Row {
    fn paint_name(&self, ctx: &mut PaintCtx<'_>, palette: &Palette, line: &Line<'_>, rect: Rect) {
        let name = TextRun::new(line.name.to_owned(), NAME).medium().no_wrap();
        let width = measure_text(&name, None).width;
        ctx.clipped(RoundedRect::new(rect, 0.0), |ctx| {
            ctx.draw_text(rect, name, palette.ink(self.selected));
        });

        let x = rect.origin.x + width + GAP;
        let room = Rect::new(
            x,
            rect.origin.y,
            (rect.max_x() - x).max(0.0),
            rect.size.height,
        );
        let version = secondary(line.version);
        ctx.clipped(RoundedRect::new(room, 0.0), |ctx| {
            ctx.draw_text(room, version, palette.text_muted);
        });
    }

    fn paint_trailing(&self, ctx: &mut PaintCtx<'_>, tint: Color, text: &str, rect: Rect) {
        let run = secondary(text);
        let width = measure_text(&run, None).width;
        let placed = Rect::new(
            rect.max_x() - width.min(rect.size.width),
            rect.origin.y,
            width.min(rect.size.width),
            rect.size.height,
        );
        ctx.clipped(RoundedRect::new(placed, 0.0), |ctx| {
            ctx.draw_text(placed, run, tint);
        });
    }
}

fn secondary(text: &str) -> TextRun {
    TextRun::new(text.to_owned(), SECONDARY).no_wrap()
}

impl Widget for Row {
    type State = ();

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let Some(line) = self.listing.line(self.index) else {
            return;
        };
        let palette = palette();
        let rect = ctx.bounds();

        if self.selected {
            ctx.draw_quad(Quad::new(rect).fill(palette.selection));
        } else if ctx.hovered() {
            ctx.draw_quad(Quad::new(rect).fill(palette.overlay.with_alpha(HOVERED)));
        } else {
            ctx.draw_quad(
                Quad::new(Rect::new(
                    rect.origin.x + PAD,
                    rect.max_y() - RULE,
                    (rect.size.width - PAD * 2.0).max(0.0),
                    RULE,
                ))
                .fill(palette.outline),
            );
        }

        ctx.draw_icon(
            Rect::new(rect.origin.x + PAD, rect.origin.y, ICON, rect.size.height),
            IconGlyph::from(line.mark.symbol()).spec(ICON),
            line.mark.tint(&palette),
        );

        let left = rect.origin.x + PAD + ICON + GAP;
        let right = rect.max_x() - PAD - TRAILING;
        let (top, bottom) = (rect.origin.y + 10.0, rect.origin.y + 30.0);

        self.paint_name(
            ctx,
            &palette,
            &line,
            Rect::new(left, top, (right - GAP - left).max(0.0), 18.0),
        );
        let description = secondary(line.description);
        let room = Rect::new(left, bottom, (right - GAP - left).max(0.0), 16.0);
        ctx.clipped(RoundedRect::new(room, 0.0), |ctx| {
            ctx.draw_text(room, description, palette.text_muted);
        });

        if let Some(label) = line.mark.label() {
            self.paint_trailing(
                ctx,
                line.mark.tint(&palette),
                &label,
                Rect::new(right, top, TRAILING, 18.0),
            );
        }
        let set = line.set;
        if !set.is_empty() {
            self.paint_trailing(
                ctx,
                palette.text_muted,
                set,
                Rect::new(right, bottom, TRAILING, 16.0),
            );
        }
    }

    fn event(&self, (): &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        let Event::PointerPressed {
            button: PointerButton::Primary,
            ..
        } = *event
        else {
            return;
        };
        ctx.request_focus();
        let index = self.index;
        ctx.run(move |manager: &mut Manager| manager.select(index));
        ctx.set_handled();
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER
    }
}
