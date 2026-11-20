use std::cell::Cell;
use std::rc::Rc;

use rikka_types::{Point, Rect};
use rikka_ui::{
    Accept, Action, Behavior, Corner, DragAction, DragOffer, Element, Event, EventCtx, IntoHandler,
    Layout, Length, PaintCtx, Quad, Style, View, Widget, motion,
};

const TINT: f32 = 0.18;
const RIM: f32 = 2.0;

type Accepts = Rc<dyn Fn(&DragOffer) -> Option<Accept>>;

#[must_use]
#[derive(Widget)]
pub struct DropZone {
    content: Cell<Option<Element>>,
    accepts: Option<Accepts>,
    on_drop: Action<(DragOffer, DragAction)>,
    content_first: bool,
    #[style]
    style: Style,
}

pub fn drop_zone(content: impl View) -> DropZone {
    DropZone {
        content: Cell::new(Some(content.build())),
        accepts: None,
        on_drop: Action::default(),
        content_first: false,
        style: Style::default(),
    }
}

impl DropZone {
    pub fn accepts(mut self, accepts: impl Fn(&DragOffer) -> Option<Accept> + 'static) -> Self {
        self.accepts = Some(Rc::new(accepts));
        self
    }

    pub fn on_drop<Marker>(
        mut self,
        on_drop: impl IntoHandler<(DragOffer, DragAction), (), Marker>,
    ) -> Self {
        self.on_drop = Action::new(on_drop);
        self
    }

    pub fn content_first(mut self, content_first: bool) -> Self {
        self.content_first = content_first;
        self
    }

    fn answer(&self, offer: &DragOffer) -> Option<Accept> {
        self.on_drop.handler()?;
        (self.accepts.as_ref()?)(offer)
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct DropZoneState {
    hovered: bool,
    shown: bool,
}

impl Widget for DropZone {
    type State = DropZoneState;

    fn layout(&self) -> Layout {
        Layout::column().with(&self.style)
    }

    fn build(&self, state: &mut DropZoneState) -> Vec<Element> {
        state.shown = state.hovered;
        let content = self.content.take().expect("content already taken");
        match state.hovered {
            true => vec![content, Glow.build()],
            false => vec![content],
        }
    }

    fn paint(&self, state: &mut DropZoneState, ctx: &mut PaintCtx<'_>) {
        if state.shown != state.hovered {
            ctx.request_view();
        }
    }

    fn event(&self, state: &mut DropZoneState, event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::DragMoved { offer, .. } => {
                let Some(accept) = self.answer(offer) else {
                    return;
                };
                ctx.accept_drag(accept);
                ctx.set_handled();
                if !state.hovered {
                    state.hovered = true;
                    ctx.request_paint();
                }
            }
            Event::DragLeft => {
                if std::mem::take(&mut state.hovered) {
                    ctx.request_paint();
                }
            }
            Event::DragDropped { offer, action, .. } => {
                state.hovered = false;
                ctx.request_paint();
                if self.answer(offer).is_none() {
                    return;
                }
                ctx.set_handled();
                ctx.act(&self.on_drop, (offer.clone(), *action));
            }
            _ => {}
        }
    }

    fn behavior(&self, _state: &DropZoneState) -> Behavior {
        Behavior::CATCHES_DROPS.when(!self.content_first)
    }
}

#[derive(Widget)]
struct Glow;

impl Widget for Glow {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::FILL, Length::FILL)
            .placed_at(Point::ZERO)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let shown = ctx.play("shown", motion::HOVER);
        let (selection, accent) = {
            let palette = ctx.palette();
            (palette.selection, palette.accent)
        };
        ctx.with_opacity(shown, |ctx| {
            ctx.draw_quad(
                Quad::new(bounds)
                    .fill(selection.scale_alpha(TINT))
                    .stroke(RIM, accent)
                    .radius(Corner::Item),
            );
        });
    }

    fn hit_test(&self, _bounds: Rect, _point: Point) -> bool {
        false
    }
}
