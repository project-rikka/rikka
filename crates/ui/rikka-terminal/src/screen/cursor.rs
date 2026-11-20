use std::cell::RefCell;
use std::rc::Rc;

use alacritty_terminal::vte::ansi::CursorShape;
use rikka_types::{Color, Point, Rect, RoundedRect};
use rikka_ui::text::{Blink, TextRun};
use rikka_ui::{Layout, Length, NodeHandle, PaintCtx, Quad, Widget};

const RULE: f32 = 1.0;
const CARET: f32 = 2.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Mark {
    pub(crate) cell: Rect,
    pub(crate) shape: CursorShape,
    pub(crate) color: Color,
    pub(crate) focused: bool,
    pub(crate) blinking: bool,
    pub(crate) under: Option<(TextRun, Color)>,
}

#[derive(Default)]
pub(crate) struct CursorCell {
    mark: RefCell<Option<Mark>>,
    painter: RefCell<Option<NodeHandle>>,
}

impl CursorCell {
    pub(crate) fn show(&self, mark: Option<Mark>) {
        if *self.mark.borrow() == mark {
            return;
        }
        *self.mark.borrow_mut() = mark;
        if let Some(painter) = self.painter.borrow().as_ref() {
            painter.repaint();
        }
    }
}

impl std::fmt::Debug for CursorCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CursorCell")
            .field("mark", &self.mark.borrow())
            .finish_non_exhaustive()
    }
}

pub(crate) struct Cursor {
    shown: Rc<CursorCell>,
}

impl Cursor {
    pub(crate) fn new(shown: &Rc<CursorCell>) -> Cursor {
        Cursor {
            shown: Rc::clone(shown),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct CursorState {
    blink: Blink,
    last_mark: Option<(Rect, bool)>,
}

impl Widget for Cursor {
    type State = CursorState;

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::FILL, Length::FILL)
            .placed_at(Point::ZERO)
    }

    fn hit_test(&self, _bounds: Rect, _point: Point) -> bool {
        false
    }

    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.shown, &other.shown)
    }

    fn paint(&self, state: &mut CursorState, ctx: &mut PaintCtx<'_>) {
        *self.shown.painter.borrow_mut() = Some(ctx.handle());
        let mark = self.shown.mark.borrow();
        let Some(mark) = mark.as_ref() else {
            state.last_mark = None;
            return;
        };
        let last_mark = (mark.cell, mark.focused);
        if state.last_mark != Some(last_mark) {
            state.last_mark = Some(last_mark);
            state.blink.restart();
        }
        let alpha = match mark.focused && mark.blinking {
            true => state.blink.alpha(ctx),
            false => 1.0,
        };
        if alpha <= 0.0 {
            return;
        }
        let bounds = ctx.bounds();
        let cell = mark.cell.translate(bounds.origin);
        ctx.clipped(RoundedRect::new(bounds, 0.0), |ctx| {
            ctx.with_opacity(alpha, |ctx| draw(ctx, mark, cell));
        });
    }
}

fn draw(ctx: &mut PaintCtx<'_>, mark: &Mark, cell: Rect) {
    let color = mark.color;
    if !mark.focused {
        ctx.draw_quad(Quad::new(cell).fill(Color::TRANSPARENT).stroke(RULE, color));
        return;
    }
    match mark.shape {
        CursorShape::Beam => {
            let beam = Rect::new(cell.origin.x, cell.origin.y, CARET, cell.size.height);
            ctx.draw_quad(Quad::new(beam).fill(color));
        }
        CursorShape::Underline => {
            let rule = Rect::new(cell.origin.x, cell.max_y() - CARET, cell.size.width, CARET);
            ctx.draw_quad(Quad::new(rule).fill(color));
        }
        CursorShape::HollowBlock => {
            ctx.draw_quad(Quad::new(cell).fill(Color::TRANSPARENT).stroke(RULE, color));
        }
        _ => {
            ctx.draw_quad(Quad::new(cell).fill(color));
            if let Some((run, ink)) = &mark.under {
                ctx.draw_text(cell, run.clone(), *ink);
            }
        }
    }
}
