use std::sync::Arc;

use rikka_theme::Corner;
use rikka_types::{PointerButton, RoundedRect, WindowView};
use rikka_ui::{
    Behavior, Curve, Element, Event, EventCtx, Handler, IntoHandler, Layout, Length, PaintCtx,
    Quad, Size, Spring, View, Widget,
};

const RING: f32 = 2.0;
const SHOWN: Curve = Curve::Spring(Spring::DEFAULT_EFFECTS);

pub(crate) struct Thumbnail {
    window: Arc<str>,
    size: Size,
    on_press: Handler,
}

#[derive(Debug, Default)]
pub(crate) struct ThumbnailState {
    hovered: bool,
}

#[must_use]
pub(crate) fn thumbnail<Marker>(
    window: &str,
    size: Size,
    on_press: impl IntoHandler<(), (), Marker>,
) -> Thumbnail {
    Thumbnail {
        window: Arc::from(window),
        size,
        on_press: on_press.into_handler(),
    }
}

impl Widget for Thumbnail {
    type State = ThumbnailState;

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::px(self.size.width), Length::px(self.size.height))
            .fixed()
    }

    fn paint(&self, state: &mut ThumbnailState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let palette = ctx.palette();
        let shape = RoundedRect::new(bounds, Corner::Item.radius());
        ctx.draw_quad(
            Quad::new(bounds)
                .fill(palette.surface_variant)
                .radius(Corner::Item.radius()),
        );
        ctx.draw_window(WindowView::new(Arc::clone(&self.window), bounds).clip(shape));
        let ring = ctx.animate_curve("ring", if state.hovered { 1.0 } else { 0.0 }, SHOWN);
        if ring > f32::EPSILON {
            ctx.draw_quad(
                Quad::new(bounds.inflate(RING))
                    .stroke(RING, palette.accent.scale_alpha(ring))
                    .radius(Corner::Item.radius() + RING),
            );
        }
    }

    fn event(&self, state: &mut ThumbnailState, event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::PointerEntered => {
                state.hovered = true;
                ctx.request_paint();
            }
            Event::PointerExited | Event::PointerLeft => {
                state.hovered = false;
                ctx.request_paint();
            }
            Event::PointerPressed {
                button: PointerButton::Primary,
                ..
            } => {
                ctx.set_handled();
                ctx.call(&self.on_press, ());
            }
            _ => {}
        }
    }

    fn behavior(&self, _state: &ThumbnailState) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn debug_name(&self) -> &'static str {
        "TaskThumbnail"
    }
}

impl std::fmt::Debug for Thumbnail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Thumbnail")
            .field("window", &self.window)
            .finish_non_exhaustive()
    }
}

impl View for Thumbnail {
    fn build(self) -> Element {
        Element::new(self)
    }
}
