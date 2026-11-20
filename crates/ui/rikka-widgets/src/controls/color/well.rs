use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use rikka_types::{Color, Key, Point, Rect, RoundedRect};
use rikka_ui::{
    Action, Anchor, Behavior, Element, Event, EventCtx, Handler, IntoHandler, Layout, Length,
    NodeHandle, PaintCtx, Quad, Style, Styled, View, Widget, motion,
};

use crate::blocks::popover::popover;
use crate::blocks::spacer::spacer;
use crate::control::ControlSize;
use crate::controls::color::paint_swatch;
use crate::controls::color::picker::color_picker;
use crate::controls::press::Press;

const WIDTH: f32 = 1.5;
const INSET: f32 = 4.0;
const DROP: f32 = 4.0;
const ANCHOR: &str = "rikka.color_well";

static WELLS: AtomicU64 = AtomicU64::new(0);

#[must_use]
#[derive(Widget)]
pub struct ColorWell {
    #[debug]
    color: Color,
    alpha: bool,
    size: ControlSize,
    enabled: bool,
    on_change: Action<(Color,)>,
    on_commit: Action<(Color,)>,
    #[style]
    style: Style,
}

pub fn color_well(color: Color) -> ColorWell {
    ColorWell {
        color,
        alpha: true,
        size: ControlSize::default(),
        enabled: true,
        on_change: Action::default(),
        on_commit: Action::default(),
        style: Style::default(),
    }
}

impl ColorWell {
    pub fn alpha(mut self, alpha: bool) -> Self {
        self.alpha = alpha;
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn on_change<Marker>(mut self, on_change: impl IntoHandler<(Color,), (), Marker>) -> Self {
        self.on_change = Action::new(on_change);
        self
    }

    pub fn on_commit<Marker>(mut self, on_commit: impl IntoHandler<(Color,), (), Marker>) -> Self {
        self.on_commit = Action::new(on_commit);
        self
    }

    fn is_active(&self) -> bool {
        self.on_change.active(self.enabled)
    }

    fn popover(&self, state: &ColorWellState) -> Element {
        let open = Rc::clone(&state.open);
        let well = NodeHandle::current();
        let dismiss = Handler::relaying(move |_, ()| {
            open.set(false);
            well.rebuild();
            well.repaint();
            Some(())
        });
        let mut picker = color_picker(self.color).alpha(self.alpha);
        if let Some(on_change) = self.on_change.handler() {
            picker = picker.on_change(on_change.clone());
        }
        if let Some(on_commit) = self.on_commit.handler() {
            picker = picker.on_commit(on_commit.clone());
        }
        popover(picker)
            .on_dismiss(dismiss)
            .build()
            .anchored(Anchor::below((ANCHOR, state.key)).offset(0.0, DROP))
    }

    fn toggle(&self, state: &ColorWellState, open: bool, ctx: &mut EventCtx<'_>) {
        if state.open.replace(open) != open {
            ctx.handle().rebuild();
            ctx.request_paint();
        }
    }
}

#[doc(hidden)]
pub struct ColorWellState {
    key: u64,
    open: Rc<Cell<bool>>,
    press: Press,
}

impl Default for ColorWellState {
    fn default() -> Self {
        Self {
            key: WELLS.fetch_add(1, Ordering::Relaxed),
            open: Rc::default(),
            press: Press::default(),
        }
    }
}

impl std::fmt::Debug for ColorWellState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColorWellState")
            .field("open", &self.open.get())
            .finish_non_exhaustive()
    }
}

impl Widget for ColorWell {
    type State = ColorWellState;

    fn layout(&self) -> Layout {
        let height = self.size.height();
        Layout::row()
            .size(Length::px((height * WIDTH).round()), Length::px(height))
            .fixed()
            .with(&self.style)
    }

    fn behavior(&self, _state: &ColorWellState) -> Behavior {
        Behavior::TRACKS_HOVER | Behavior::FOCUSABLE.when(self.is_active())
    }

    fn build(&self, state: &mut ColorWellState) -> Vec<Element> {
        let anchor = spacer()
            .width(Length::FILL)
            .height(Length::FILL)
            .build()
            .keyed((ANCHOR, state.key));
        let mut children = vec![anchor];
        if state.open.get() && self.is_active() {
            children.push(self.popover(state));
        }
        children
    }

    fn paint(&self, state: &mut ColorWellState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let active = self.is_active();
        let open = state.open.get() && active;
        let palette = ctx.theme().palette();
        let (radius, border, layer, dimmed) = {
            let theme = ctx.theme();
            let border = match () {
                _ if !active => theme.disabled(palette.outline),
                _ if ctx.focused() || open => palette.accent,
                _ => palette.outline,
            };
            let layer = match () {
                _ if active && state.press.is_down() => Some(theme.press_layer()),
                _ if active && ctx.hovered() => Some(theme.hover_layer()),
                _ => None,
            };
            let dimmed = theme.dim(self.color, !active);
            (self.size.radius(), border, layer, dimmed)
        };
        let border = ctx.animate_curve("border", border, motion::COLOR);
        ctx.draw_quad(
            Quad::new(bounds)
                .fill(palette.surface_variant)
                .stroke(1.0, border)
                .radius(radius),
        );
        let inner = Rect::new(
            bounds.x() + INSET,
            bounds.y() + INSET,
            (bounds.width() - INSET * 2.0).max(0.0),
            (bounds.height() - INSET * 2.0).max(0.0),
        );
        let inner_radius = (radius - INSET / 2.0).max(0.0);
        paint_swatch(ctx, RoundedRect::new(inner, inner_radius), dimmed);
        if let Some(layer) = layer {
            ctx.draw_quad(Quad::new(bounds).fill(layer).radius(radius));
        }
    }

    fn event(&self, state: &mut ColorWellState, event: &Event, ctx: &mut EventCtx<'_>) {
        if !self.is_active() {
            return;
        }
        let outside = |at: Point| !ctx.bounds().contains(at);
        match *event {
            Event::PointerPressed { position, .. } if outside(position) => return,
            Event::KeyPressed {
                key: Key::Escape, ..
            } if ctx.focused() && state.open.get() => {
                ctx.set_handled();
                self.toggle(state, false, ctx);
                return;
            }
            _ => {}
        }
        if state.press.update(event, true, ctx) {
            let open = !state.open.get();
            self.toggle(state, open, ctx);
        }
    }
}
