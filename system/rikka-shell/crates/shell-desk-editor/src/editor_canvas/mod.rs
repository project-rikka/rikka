pub(crate) mod frame;
mod input;
mod paint;

use std::sync::Arc;

use desk_layout::{Area, Content, Layout, Projection, Size as Step, WidgetId};
pub(crate) use paint::card_radius;
use rikka_types::{Point, Rect};
use rikka_ui::{
    Behavior, Element, Event, EventCtx, ImageSource, Layout as Flow, Length, PaintCtx, Widget,
};
use shell_kit::DesktopFrost;
use shell_kit::taskbar::Shape;

use crate::editing::Editing;
use crate::session::{Offer, Session, Tool};
use crate::wallpaper_loader::Pictures;

pub(crate) struct Sheet {
    pub(crate) area: Area,
    pub(crate) layout: Layout,
    pub(super) editable: bool,
    pub(crate) selected: Option<WidgetId>,
    pub(crate) thumbs: Vec<Thumb>,
    pub(crate) current: Option<usize>,
    pub(crate) pictures: Pictures,
    pub(crate) taskbar: Option<Shape>,
    pub(crate) panel: Rect,
    pub(crate) session: Session,
    pub(crate) editing: Editing,
    pub(crate) behind: Vec<(Arc<str>, Rect)>,
    pub(crate) frost: DesktopFrost,
    pub(crate) output: Arc<str>,
}

pub(crate) struct Thumb {
    pub(crate) preview: Option<ImageSource>,
    pub(crate) motion: bool,
    pub(super) silhouettes: Vec<Rect>,
}

struct EditorCanvas {
    sheet: Sheet,
}

#[must_use]
pub(super) fn editor_canvas(sheet: Sheet, layers: Vec<Element>) -> Element {
    Element::with_children(EditorCanvas { sheet }, layers)
}

#[derive(Debug, Default)]
struct EditorCanvasState {
    hits: Hits,
    settled: bool,
    scroll: f32,
    last_drawn: Option<usize>,
    thumb: Option<usize>,
    press: Option<Press>,
    drag: Option<Drag>,
    alt: bool,
}

#[derive(Debug, Default)]
struct Hits {
    card: Rect,
    projection: Option<Projection>,
    thumbs: Vec<(usize, Rect)>,
    strip: Rect,
    overflow: f32,
}

#[derive(Clone, Debug)]
struct Press {
    at: Point,
    on: Pressed,
}

#[derive(Clone, Debug, PartialEq)]
enum Pressed {
    Widget { id: WidgetId, rect: Rect },
    Offer(Offer),
    Thumb(usize),
    Tool(Tool),
    Card,
    Panel,
    Away,
}

#[derive(Clone, Debug)]
struct Drag {
    id: WidgetId,
    size: Step,
    grab: Point,
    adding: Option<Content>,
    rect: Option<Rect>,
    pointer: Point,
}

impl Widget for EditorCanvas {
    type State = EditorCanvasState;

    fn layout(&self) -> Flow {
        Flow::column().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, state: &mut EditorCanvasState, ctx: &mut PaintCtx<'_>) {
        self.draw(state, ctx);
    }

    fn event(&self, state: &mut EditorCanvasState, event: &Event, ctx: &mut EventCtx<'_>) {
        self.respond(state, event, ctx);
    }

    fn behavior(&self, _state: &EditorCanvasState) -> Behavior {
        Behavior::FOCUSABLE | Behavior::AUTOFOCUS | Behavior::TAKES_TAB
    }

    fn relocated(&self, state: &mut EditorCanvasState, by: Point) {
        state.hits.card = state.hits.card.translate(by);
        for (_, rect) in &mut state.hits.thumbs {
            *rect = rect.translate(by);
        }
    }

    fn debug_name(&self) -> &'static str {
        "DeskEasel"
    }
}

impl std::fmt::Debug for EditorCanvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorCanvas")
            .field("widgets", &self.sheet.layout.widgets.len())
            .field("selected", &self.sheet.selected)
            .finish_non_exhaustive()
    }
}
