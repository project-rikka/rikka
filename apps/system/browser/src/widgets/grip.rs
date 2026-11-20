use rikka_app::prelude::*;
use rikka_types::DragActions;
use rikka_ui::{Action, DRAG_SLOP, DragEnd, DragStart, Offer, motion};

use crate::widgets::row_drag;
use browser_model::Row;

pub(crate) struct Grip {
    highlighted: bool,
    press: Action,
    middle: Action,
    aim: Action,
    hover: Action<(bool,)>,
    ended: Action<(DragEnd,)>,
    drag_item: Option<DragItem>,
    style: Style,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DragItem {
    pub(crate) row: Row,
    pub(crate) symbol: MaterialSymbol,
    pub(crate) label: String,
}

pub(crate) fn grip(content: impl View) -> GripView {
    GripView {
        grip: Grip {
            highlighted: false,
            press: Action::default(),
            middle: Action::default(),
            aim: Action::default(),
            hover: Action::default(),
            ended: Action::default(),
            drag_item: None,
            style: Style::default(),
        },
        content: content.build(),
        tip: None,
    }
}

pub(crate) struct GripView {
    grip: Grip,
    content: Element,
    tip: Option<Tooltip>,
}

impl GripView {
    pub(crate) fn highlighted(mut self, highlighted: bool) -> Self {
        self.grip.highlighted = highlighted;
        self
    }

    pub(crate) fn on_press<M>(mut self, press: impl IntoHandler<(), (), M>) -> Self {
        self.grip.press = Action::new(press);
        self
    }

    pub(crate) fn middle<M>(mut self, middle: impl IntoHandler<(), (), M>) -> Self {
        self.grip.middle = Action::new(middle);
        self
    }

    pub(crate) fn aim<M>(mut self, aim: impl IntoHandler<(), (), M>) -> Self {
        self.grip.aim = Action::new(aim);
        self
    }

    pub(crate) fn hover<M>(mut self, hover: impl IntoHandler<(bool,), (), M>) -> Self {
        self.grip.hover = Action::new(hover);
        self
    }

    pub(crate) fn drag_item(mut self, drag_item: DragItem) -> Self {
        self.grip.drag_item = Some(drag_item);
        self
    }

    pub(crate) fn tip(mut self, label: impl Into<String>, side: TooltipSide) -> Self {
        self.tip = Some(Tooltip::new(label.into()).side(side));
        self
    }

    pub(crate) fn ended<M>(mut self, ended: impl IntoHandler<(DragEnd,), (), M>) -> Self {
        self.grip.ended = Action::new(ended);
        self
    }
}

impl Styled for GripView {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.grip.style
    }
}

impl View for GripView {
    fn build(self) -> Element {
        let element = Element::with_children(self.grip, vec![self.content]);
        match self.tip {
            Some(tip) => element.tooltip(tip),
            None => element,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct GripState {
    armed: Option<Point>,
}

impl Widget for Grip {
    type State = GripState;

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn behavior(&self, _state: &GripState) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn paint(&self, _state: &mut GripState, ctx: &mut PaintCtx<'_>) {
        let palette = ctx.theme().palette();
        let fill = match (self.highlighted, ctx.hovered()) {
            (true, _) => palette.text.with_alpha(0.12),
            (false, true) => palette.text.with_alpha(0.06),
            (false, false) => palette.text.with_alpha(0.0),
        };
        let fill = ctx.animate_curve("fill", fill, motion::HOVER);
        let bounds = ctx.bounds();
        ctx.draw_quad(Quad::new(bounds).fill(fill).radius(Corner::Item));
    }

    fn event(&self, state: &mut GripState, event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::PointerEntered => ctx.act(&self.hover, (true,)),
            Event::PointerExited => ctx.act(&self.hover, (false,)),
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                state.armed = Some(*position);
                ctx.capture_pointer();
                ctx.set_handled();
            }
            Event::PointerPressed {
                button: PointerButton::Middle,
                ..
            } => {
                ctx.act(&self.middle, ());
                ctx.set_handled();
            }
            Event::PointerPressed {
                button: PointerButton::Secondary,
                ..
            } => ctx.act(&self.aim, ()),
            Event::PointerMoved(at) => {
                let Some(origin) = state.armed else {
                    return;
                };
                let travelled = (at.x - origin.x).hypot(at.y - origin.y);
                if travelled < DRAG_SLOP {
                    return;
                }
                state.armed = None;
                ctx.release_pointer();
                if let Some(drag_item) = &self.drag_item {
                    let theme = ctx.theme().clone();
                    let offer =
                        Offer::new().with(row_drag::mime(), row_drag::encode(drag_item.row));
                    let icon = drag_chip(theme, drag_item.symbol, &drag_item.label);
                    ctx.start_drag(DragStart::new(offer, icon).actions(DragActions::MOVE));
                    ctx.set_handled();
                }
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } => {
                if state.armed.take().is_some() {
                    ctx.release_pointer();
                    ctx.act(&self.press, ());
                    ctx.set_handled();
                }
            }
            Event::DragEnded(end) => ctx.act(&self.ended, (*end,)),
            _ => {}
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.highlighted == other.highlighted
            && self.press == other.press
            && self.middle == other.middle
            && self.aim == other.aim
            && self.hover == other.hover
            && self.ended == other.ended
            && self.drag_item == other.drag_item
            && self.style == other.style
    }

    fn debug_name(&self) -> &'static str {
        "Grip"
    }
}
