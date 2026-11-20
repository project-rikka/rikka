use std::sync::Arc;

use rikka_text::TextLayout;
use rikka_theme::{Corner, Theme};
use rikka_types::{Point, Rect, Size};

use crate::motion;
use crate::paint::{PaintCtx, Quad};
use crate::runtime::tips::{Bubble, Entrance};
use crate::style::Space;
use crate::text::{TextRole, TextRun};
use crate::view::{DropdownPlacement, Tooltip, TooltipSide, VIEWPORT_MARGIN, clamp_into};

const SCOPE: &str = "rikka:tooltip";
const SHOWN: &str = "shown";
const WIDEST: f32 = 320.0;
const GAP: f32 = Space::TIGHT.px();
const PADDING_X: f32 = Space::SNUG.px();
const PADDING_Y: f32 = Space::TIGHT.px();
const SHORTCUT_GAP: f32 = Space::NORMAL.px();
const OUTLINE: f32 = 1.0;

pub(crate) fn paint(ctx: &mut PaintCtx<'_>, bubble: &Bubble) {
    ctx.motion_scope(SCOPE, |ctx| {
        match bubble.entrance {
            Some(Entrance::Grow) => ctx.restart(SHOWN),
            Some(Entrance::AtOnce) => ctx.jump(SHOWN, 1.0_f32),
            None => {}
        }
        let shown = ctx.progress(SHOWN, bubble.open, motion::POP, motion::POP_OUT);
        if shown <= 0.0 {
            return;
        }
        let words = Words::new(&bubble.tooltip, ctx.theme());
        let placed = place(
            bubble.tooltip.placement(),
            bubble.anchor,
            words.size,
            ctx.viewport(),
        );
        let rect = Rect::new(
            ctx.snap(placed.rect().x()),
            ctx.snap(placed.rect().y()),
            placed.rect().width(),
            placed.rect().height(),
        );
        ctx.topmost(|ctx| {
            ctx.popped(shown, placed.pivot(), |ctx| words.draw(ctx, rect));
        });
    });
}

struct Words {
    label: (TextRun, Arc<TextLayout>),
    shortcut: Option<(TextRun, Arc<TextLayout>)>,
    size: Size,
}

impl Words {
    fn new(tooltip: &Tooltip, theme: &Theme) -> Self {
        let font_size = theme.typography().body_size * TextRole::Caption.scale();
        let shortcut = tooltip.chord().map(|chord| {
            let run = TextRun::new(chord.to_string(), font_size).no_wrap();
            let laid = rikka_text::layout_approx(run.as_shape(), None);
            (run, laid)
        });
        let beside = shortcut
            .as_ref()
            .map_or(0.0, |(_, laid)| laid.size().width + SHORTCUT_GAP);
        let room = (WIDEST - PADDING_X * 2.0 - beside).max(0.0);
        let run = TextRun::new(tooltip.label(), font_size);
        let laid = rikka_text::layout_approx(run.as_shape(), Some(room));
        let label = laid.size();
        let tallest = shortcut.as_ref().map_or(label.height, |(_, laid)| {
            label.height.max(laid.size().height)
        });
        Self {
            label: (run, laid),
            shortcut,
            size: Size::new(
                label.width + beside + PADDING_X * 2.0,
                tallest + PADDING_Y * 2.0,
            ),
        }
    }

    fn draw(&self, ctx: &mut PaintCtx<'_>, rect: Rect) {
        let palette = ctx.theme().palette();
        ctx.draw_pane(
            Quad::new(rect)
                .fill(palette.surface)
                .stroke(OUTLINE, palette.outline)
                .radius(Corner::Item),
        );
        let (run, laid) = &self.label;
        let label = laid.size();
        let top = rect.y() + PADDING_Y;
        let start = rect.x() + PADDING_X;
        ctx.draw_laid_text(
            Rect::new(start, top, label.width, label.height),
            run.clone(),
            Arc::clone(laid),
            palette.text,
        );
        if let Some((run, laid)) = &self.shortcut {
            let size = laid.size();
            ctx.draw_laid_text(
                Rect::new(
                    rect.max_x() - PADDING_X - size.width,
                    top,
                    size.width,
                    size.height,
                ),
                run.clone(),
                Arc::clone(laid),
                palette.text_muted,
            );
        }
    }
}

fn place(side: TooltipSide, target: Rect, size: Size, viewport: Size) -> DropdownPlacement {
    let side = turned(side, target, size, viewport);
    let center = target.center();
    let (origin, attached) = match side {
        TooltipSide::Below => (
            Point::new(center.x - size.width / 2.0, target.max_y() + GAP),
            Point::new(center.x, target.max_y()),
        ),
        TooltipSide::Above => (
            Point::new(center.x - size.width / 2.0, target.y() - GAP - size.height),
            Point::new(center.x, target.y()),
        ),
        TooltipSide::After => (
            Point::new(target.max_x() + GAP, center.y - size.height / 2.0),
            Point::new(target.max_x(), center.y),
        ),
        TooltipSide::Before => (
            Point::new(target.x() - GAP - size.width, center.y - size.height / 2.0),
            Point::new(target.x(), center.y),
        ),
    };
    let origin = match viewport.is_empty() {
        true => origin,
        false => Point::new(
            clamp_into(origin.x, size.width, viewport.width),
            clamp_into(origin.y, size.height, viewport.height),
        ),
    };
    DropdownPlacement::hung(Rect::from_origin_size(origin, size), attached)
}

fn turned(side: TooltipSide, target: Rect, size: Size, viewport: Size) -> TooltipSide {
    if viewport.is_empty() {
        return side;
    }
    let below = viewport.height - target.max_y() - GAP - VIEWPORT_MARGIN;
    let above = target.y() - GAP - VIEWPORT_MARGIN;
    let after = viewport.width - target.max_x() - GAP - VIEWPORT_MARGIN;
    let before = target.x() - GAP - VIEWPORT_MARGIN;
    let (room, other, opposite) = match side {
        TooltipSide::Below => (below, above, TooltipSide::Above),
        TooltipSide::Above => (above, below, TooltipSide::Below),
        TooltipSide::After => (after, before, TooltipSide::Before),
        TooltipSide::Before => (before, after, TooltipSide::After),
    };
    let needed = match side {
        TooltipSide::Below | TooltipSide::Above => size.height,
        TooltipSide::After | TooltipSide::Before => size.width,
    };
    match room < needed && other > room {
        true => opposite,
        false => side,
    }
}
