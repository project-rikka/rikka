use std::any::Any;
use std::cell::Cell;

use rikka_ui::{
    Accept, DragOffer, Element, Event, EventCtx, Handler, IntoHandler, Layout, Length, PaintCtx,
    Style, View, Widget,
};

use crate::navigation::panes::highlight::highlight;
use crate::navigation::panes::{DropTarget, landing_at};
use crate::navigation::tabs;

#[must_use]
#[derive(Widget)]
pub struct Pane {
    content: Cell<Option<Element>>,
    accepts: Option<(String, Handler<(DropTarget,)>)>,
    #[style]
    style: Style,
}

pub fn pane(content: impl View) -> Pane {
    Pane {
        content: Cell::new(Some(content.build())),
        accepts: None,
        style: Style::default(),
    }
}

impl Pane {
    pub fn accepting<T: Any, Marker>(
        mut self,
        group: impl Into<String>,
        land: impl IntoHandler<(DropTarget, T), (), Marker>,
    ) -> Self {
        self.accepts = Some((group.into(), tabs::payload_handler(land)));
        self
    }

    fn mime(&self, offer: &DragOffer) -> Option<String> {
        let (group, _) = self.accepts.as_ref()?;
        Some(tabs::group_mime(group)).filter(|mime| offer.offers(mime))
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct PaneState {
    landing: Option<DropTarget>,
    shown: Option<DropTarget>,
}

impl Widget for Pane {
    type State = PaneState;

    fn layout(&self) -> Layout {
        Layout::column()
            .size(Length::FILL, Length::FILL)
            .with(&self.style)
    }

    fn build(&self, state: &mut PaneState) -> Vec<Element> {
        state.shown = state.landing;
        let content = self.content.take().expect("content already taken");
        match state.landing {
            Some(landing) => vec![content, highlight(landing).build()],
            None => vec![content],
        }
    }

    fn paint(&self, state: &mut PaneState, ctx: &mut PaintCtx<'_>) {
        if state.shown != state.landing {
            ctx.request_view();
        }
    }

    fn event(&self, state: &mut PaneState, event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::DragMoved { position, offer } => {
                let Some(mime) = self.mime(offer) else {
                    return;
                };
                let landing = landing_at(ctx.bounds(), *position);
                if state.landing != Some(landing) {
                    state.landing = Some(landing);
                    ctx.request_paint();
                }
                ctx.accept_drag(Accept::moving(mime));
                ctx.set_handled();
            }
            Event::DragLeft => {
                if state.landing.take().is_some() {
                    ctx.request_paint();
                }
            }
            Event::DragDropped {
                position, offer, ..
            } => {
                if self.mime(offer).is_none() {
                    return;
                }
                let landing = landing_at(ctx.bounds(), *position);
                state.landing = None;
                ctx.set_handled();
                ctx.request_paint();
                if let Some((_, land)) = &self.accepts {
                    ctx.call(land, (landing,));
                }
            }
            _ => {}
        }
    }
}
