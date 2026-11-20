use rikka_ui::palette;

use rikka_app::prelude::{
    Behavior, Command, Event, EventCtx, ImageSource, Key, PaintCtx, PanZoom, PointerButton, Quad,
    RoundedRect, Sprite, Widget, Zoom,
};
use rikka_types::{Point, Rect, Size};
use viewer_media::{Extent, Step};

use crate::screen::picture::{PROXY_SIDE, Picture, Proxy};
use crate::viewer::Viewer;

const STEP: f32 = 1.25;
const MIN: f32 = 0.02;
const MAX: f32 = 16.0;

#[derive(Debug)]
pub(crate) struct Canvas {
    pub(crate) source: ImageSource,
    pub(crate) proxy: Proxy,
    pub(crate) extent: Extent,
    pub(crate) zoom: Zoom,
}

#[derive(Debug)]
pub(crate) struct CanvasState {
    pan_zoom: PanZoom,
    grab: Option<Point>,
    applied: Option<Zoom>,
}

impl Default for CanvasState {
    fn default() -> CanvasState {
        CanvasState {
            pan_zoom: PanZoom::new(MIN, MAX),
            grab: None,
            applied: None,
        }
    }
}

impl Widget for Canvas {
    type State = CanvasState;

    fn paint(&self, state: &mut CanvasState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        state.pan_zoom.meet(bounds.size);
        let extent = Size::new(self.extent.width as f32, self.extent.height as f32);
        if state.pan_zoom.show(extent) {
            state.applied = None;
        }
        if state.applied != Some(self.zoom) {
            state.applied = Some(self.zoom);
            state.pan_zoom.reset(self.zoom);
        }

        ctx.draw_quad(Quad::new(bounds).fill(palette().background));
        let placed = state.pan_zoom.placed();
        let picture = Rect::from_origin_size(placed.origin + bounds.origin, placed.size);
        let on_screen_width = picture.width() * ctx.scale();
        ctx.clipped(RoundedRect::new(bounds, 0.0), |ctx| match &self.proxy {
            Proxy::Unneeded => ctx.draw_image(picture, self.source.clone()),
            Proxy::Making => {
                if on_screen_width > PROXY_SIDE as f32 {
                    ctx.draw_image(picture, self.source.clone());
                }
            }
            Proxy::Made { source, width } => {
                if *width as f32 >= on_screen_width {
                    ctx.draw_image(picture, source.clone());
                } else {
                    ctx.draw_sprite(
                        Sprite::new(picture, self.source.clone()).placeholder(source.clone()),
                    );
                }
            }
        });
    }

    fn event(&self, state: &mut CanvasState, event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                ctx.request_focus();
                state.grab = Some(position);
                ctx.capture_pointer();
            }
            Event::PointerMoved(position) => {
                let Some(previous) = state.grab else {
                    return;
                };
                state.grab = Some(position);
                let fitted = state.pan_zoom.is_fitted();
                state
                    .pan_zoom
                    .pan(Point::new(position.x - previous.x, position.y - previous.y));
                if fitted {
                    let zoom = state.pan_zoom.zoom();
                    state.applied = Some(zoom);
                    zoomed(ctx, zoom);
                }
                ctx.request_paint();
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } => {
                state.grab = None;
                ctx.release_pointer();
            }
            Event::Scroll {
                position, delta, ..
            } => {
                rescale(state, ctx, STEP.powf(delta.zoom()), Some(position));
            }
            Event::KeyPressed { key, modifiers, .. }
                if !modifiers.control && !modifiers.alt && !modifiers.logo =>
            {
                self.pressed(state, ctx, key);
            }
            Event::Command(command) => self.commanded(state, ctx, command),
            _ => {}
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::FOCUSABLE | Behavior::AUTOFOCUS
    }
}

impl Canvas {
    fn pressed(&self, state: &mut CanvasState, ctx: &mut EventCtx<'_>, key: Key) {
        match key {
            Key::Char('+' | '=') => self.commanded(state, ctx, Command::ZoomIn),
            Key::Char('-' | '_') => self.commanded(state, ctx, Command::ZoomOut),
            Key::Char('1') => zoomed(ctx, Zoom::Scale(1.0)),
            Key::Char('0' | 'f' | 'F') => zoomed(ctx, Zoom::Fit),
            Key::ArrowLeft | Key::PageUp => stepped(ctx, Step::Previous),
            Key::ArrowRight | Key::PageDown => stepped(ctx, Step::Next),
            _ => return,
        }
        ctx.set_handled();
    }

    fn commanded(&self, state: &mut CanvasState, ctx: &mut EventCtx<'_>, command: Command) {
        match command {
            Command::ZoomIn => rescale(state, ctx, STEP, None),
            Command::ZoomOut => rescale(state, ctx, 1.0 / STEP, None),
            Command::ZoomReset => zoomed(ctx, Zoom::Fit),
            _ => return,
        }
        ctx.set_handled();
    }
}

fn rescale(state: &mut CanvasState, ctx: &mut EventCtx<'_>, factor: f32, anchor: Option<Point>) {
    if !state.pan_zoom.zoom_by(factor, anchor) {
        return;
    }
    let zoom = state.pan_zoom.zoom();
    state.applied = Some(zoom);
    zoomed(ctx, zoom);
}

fn zoomed(ctx: &mut EventCtx<'_>, zoom: Zoom) {
    ctx.run(move |picture: &mut Picture| picture.zoomed(zoom));
}

fn stepped(ctx: &mut EventCtx<'_>, step: Step) {
    ctx.run(move |viewer: &mut Viewer| viewer.step(step));
}
