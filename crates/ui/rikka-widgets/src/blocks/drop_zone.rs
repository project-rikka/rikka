use std::cell::Cell;
use std::rc::Rc;

use rikka_types::{Point, Rect};
use rikka_ui::{
    Accept, Action, Behavior, Corner, DragAction, DragOffer, Element, Event, EventCtx, IntoHandler,
    Layout, Length, PaintCtx, Quad, Style, Styled, View, Watchers, Widget, motion,
};

const TINT: f32 = 0.18;
const RIM: f32 = 2.0;

type Accepts = Rc<dyn Fn(&DragOffer) -> Option<Accept>>;

#[must_use]
pub struct DropZone {
    content: Element,
    accepts: Option<Accepts>,
    on_drop: Action<(DragOffer, DragAction)>,
    content_first: bool,
    lit: Rc<Lit>,
    style: Style,
}

#[derive(Default)]
struct Lit {
    hovered: Cell<bool>,
    watchers: Watchers,
}

impl Lit {
    fn show(&self, hovered: bool) {
        if self.hovered.replace(hovered) != hovered {
            self.watchers.repaint();
        }
    }
}

pub fn drop_zone(content: impl View) -> DropZone {
    DropZone {
        content: content.build(),
        accepts: None,
        on_drop: Action::default(),
        content_first: false,
        lit: Rc::default(),
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

impl Styled for DropZone {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl View for DropZone {
    fn build(mut self) -> Element {
        let content = std::mem::replace(&mut self.content, Element::empty());
        let glow = Element::new(Glow {
            lit: Rc::clone(&self.lit),
        });
        Element::with_children(self, vec![content, glow])
    }
}

impl std::fmt::Debug for DropZone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DropZone")
            .field("content_first", &self.content_first)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct DropZoneState {
    hovered: bool,
}

impl Widget for DropZone {
    type State = DropZoneState;

    fn layout(&self) -> Layout {
        Layout::column().with(&self.style)
    }

    fn paint(&self, state: &mut DropZoneState, _ctx: &mut PaintCtx<'_>) {
        self.lit.show(state.hovered);
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
                    self.lit.show(true);
                }
            }
            Event::DragLeft => {
                if std::mem::take(&mut state.hovered) {
                    self.lit.show(false);
                }
            }
            Event::DragDropped { offer, action, .. } => {
                state.hovered = false;
                self.lit.show(false);
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

    fn debug_name(&self) -> &'static str {
        "DropZone"
    }
}

struct Glow {
    lit: Rc<Lit>,
}

impl Widget for Glow {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::FILL, Length::FILL)
            .placed_at(Point::ZERO)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        self.lit.watchers.watch(ctx.handle());
        let target = match self.lit.hovered.get() {
            true => 1.0,
            false => 0.0,
        };
        let shown = ctx.animate_curve("shown", target, motion::HOVER);
        if shown <= 0.0 {
            return;
        }
        let bounds = ctx.bounds();
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

    fn debug_name(&self) -> &'static str {
        "DropZoneGlow"
    }
}

impl std::fmt::Debug for Glow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Glow").finish_non_exhaustive()
    }
}
