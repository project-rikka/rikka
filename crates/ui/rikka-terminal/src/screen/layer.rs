use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rikka_types::{Color, Point, Rect, RoundedRect};
use rikka_ui::text::TextRun;
use rikka_ui::{Layout, Length, NodeHandle, PaintCtx, Quad, Widget};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Stroke {
    Fill(Rect, Color),
    Text(Rect, TextRun, Color),
}

#[derive(Debug, Default)]
pub(crate) struct Sketch(Vec<Stroke>);

impl Sketch {
    pub(crate) fn fill(&mut self, rect: Rect, color: Color) {
        self.0.push(Stroke::Fill(rect, color));
    }

    pub(crate) fn text(&mut self, rect: Rect, run: TextRun, color: Color) {
        self.0.push(Stroke::Text(rect, run, color));
    }
}

#[derive(Default)]
pub(crate) struct LayerCell {
    strokes: RefCell<Vec<Stroke>>,
    slot: Cell<i32>,
    painter: RefCell<Option<NodeHandle>>,
}

impl LayerCell {
    pub(crate) fn show(&self, sketch: &mut Sketch) {
        let mut strokes = self.strokes.borrow_mut();
        if *strokes != sketch.0 {
            std::mem::swap(&mut *strokes, &mut sketch.0);
            if let Some(painter) = self.painter.borrow().as_ref() {
                painter.repaint();
            }
        }
        sketch.0.clear();
    }

    pub(crate) fn place(&self, slot: i32) {
        if self.slot.replace(slot) == slot {
            return;
        }
        if let Some(painter) = self.painter.borrow().as_ref() {
            painter.relayout();
        }
    }
}

impl std::fmt::Debug for LayerCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayerCell")
            .field("strokes", &self.strokes.borrow().len())
            .field("slot", &self.slot.get())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Placement {
    Line { height: f32 },
    Over,
}

pub(crate) struct Layer {
    shown: Rc<LayerCell>,
    placement: Placement,
}

impl Layer {
    pub(crate) fn line(shown: &Rc<LayerCell>, height: f32) -> Layer {
        Layer {
            shown: Rc::clone(shown),
            placement: Placement::Line { height },
        }
    }

    pub(crate) fn over(shown: &Rc<LayerCell>) -> Layer {
        Layer {
            shown: Rc::clone(shown),
            placement: Placement::Over,
        }
    }
}

impl Widget for Layer {
    type State = ();

    fn layout(&self) -> Layout {
        match self.placement {
            Placement::Line { height } => Layout::row()
                .size(Length::FILL, Length::px(height))
                .placed_at(Point::new(0.0, self.shown.slot.get() as f32 * height)),
            Placement::Over => Layout::row()
                .size(Length::FILL, Length::FILL)
                .placed_at(Point::ZERO),
        }
    }

    fn hit_test(&self, _bounds: Rect, _point: Point) -> bool {
        false
    }

    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.shown, &other.shown) && self.placement == other.placement
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        *self.shown.painter.borrow_mut() = Some(ctx.handle());
        let strokes = self.shown.strokes.borrow();
        if strokes.is_empty() {
            return;
        }
        let bounds = ctx.bounds();
        ctx.clipped(RoundedRect::new(bounds, 0.0), |ctx| {
            for stroke in strokes.iter() {
                match stroke {
                    Stroke::Fill(rect, color) => {
                        ctx.draw_quad(Quad::new(rect.translate(bounds.origin)).fill(*color));
                    }
                    Stroke::Text(rect, run, color) => {
                        ctx.draw_text(rect.translate(bounds.origin), run.clone(), *color);
                    }
                }
            }
        });
    }
}
