use std::cell::Cell;
use std::rc::Rc;

use rikka_types::{Point, Rect};
use rikka_ui::scroll::bar::{Grip, Lanes, Scroll};
use rikka_ui::text::{Blink, DisplayRevision, TextRun};

use crate::navigation::menu::ContextMenu;
use crate::text::editing::assist::{Cause, Completing};
use crate::text::editing::block::Block;
use crate::text::editing::carry::DragOrigin;
use crate::text::editing::find::FindState;
use crate::text::editing::gesture::Gesture;
use crate::text::editing::rows::Rows;

#[derive(Debug, Default)]
pub(crate) struct EditingState {
    pub(crate) down: Scroll,
    pub(crate) across: Scroll,
    pub(crate) down_grip: Grip,
    pub(crate) across_grip: Grip,
    pub(crate) rows: Rows,
    pub(crate) block: Block,
    pub(crate) frame: Frame,
    pub(crate) visible: std::ops::Range<usize>,
    pub(crate) finding: FindState,
    pub(crate) lanes: Lanes,
    pub(crate) widest: f32,
    pub(crate) caret: Option<Rect>,
    pub(crate) selecting: bool,
    pub(crate) blink: Blink,
    pub(crate) blinking_at: Option<(usize, DisplayRevision)>,
    pub(crate) dragging: bool,
    pub(crate) gesture: Option<Gesture>,
    pub(crate) drag_origin: Option<DragOrigin>,
    pub(crate) lifting: Option<std::ops::Range<usize>>,
    pub(crate) dropped_on_self: bool,
    pub(crate) drop_at: Option<usize>,
    pub(crate) follow: bool,
    pub(crate) pasted: Rc<Cell<bool>>,
    pub(crate) caret_position: Option<CaretPosition>,
    pub(crate) goal: Option<f32>,
    pub(crate) template: Option<TextRun>,
    pub(crate) syntax: Option<rikka_syntax::Parsed>,
    pub(crate) coloured: bool,
    pub(crate) menu: ContextMenu,
    pub(crate) completing: Option<Completing>,
    pub(crate) cause: Option<Cause>,
}

impl EditingState {
    pub(crate) fn relocated(&mut self, by: Point) {
        self.frame = self.frame.translate(by);
        self.caret = self.caret.map(|caret| caret.translate(by));
    }

    pub(crate) fn caret_within(&self) -> Option<Rect> {
        let home = self.frame.bounds.origin;
        self.caret.map(|caret| caret.translate(Point::ZERO - home))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CaretPosition {
    pub(crate) caret: usize,
    pub(crate) revision: DisplayRevision,
    pub(crate) width: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Frame {
    pub(crate) bounds: Rect,
    pub(crate) room: Rect,
    pub(crate) area: Rect,
    pub(crate) top: f32,
    pub(crate) origin: f32,
    pub(crate) sheet_top: f32,
    pub(crate) scrolled: Point,
    pub(crate) line_height: f32,
    pub(crate) content_height: f32,
    pub(crate) content_width: f32,
}

impl Frame {
    fn translate(self, by: Point) -> Frame {
        Frame {
            bounds: self.bounds.translate(by),
            room: self.room.translate(by),
            area: self.area.translate(by),
            origin: self.origin + by.x,
            sheet_top: self.sheet_top + by.y,
            ..self
        }
    }

    pub(crate) fn placed(&self, band: Rect) -> Rect {
        Rect::new(
            self.origin + band.x(),
            self.sheet_top + band.y(),
            band.width(),
            band.height(),
        )
    }

    pub(crate) fn on_screen(&self, rect: Rect) -> Rect {
        rect.translate(self.scrolled)
    }

    pub(crate) fn within_sheet(&self, at: Point) -> Point {
        Point::new(
            at.x - self.origin - self.scrolled.x,
            at.y - self.sheet_top - self.scrolled.y,
        )
    }
}
