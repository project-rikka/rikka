use std::collections::BTreeSet;
use std::sync::Arc;

use rikka_picture::RefreshRate;
use rikka_remote_wire::{Button, InputEvent, POSITION_EXTENT, PointerMode, Position, Viewport};
use rikka_types::{
    Color, CursorShape, Modifiers, Point, PointerButton, Rect, SCROLL_LINE, ScrollDelta, Size,
};
use rikka_ui::{
    Accept, Action, Behavior, Event, EventCtx, Feed, ImageSource, PaintCtx, Quad, ScrollSource,
    Style, URI_LIST, Widget,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::session::Command;

const VALUE120: f32 = 120.0;
const KEY_Q: u32 = 16;
const KEY_S: u32 = 31;
const KEY_F: u32 = 33;
const KEY_M: u32 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Escape {
    Leave,
    Fullscreen,
    Stats,
}

#[derive(Clone, Debug)]
pub(crate) struct Sprite {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) hotspot: Point,
    pub(crate) pixels: Arc<[u8]>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Hold {
    #[default]
    Free,
    Declined,
    Chosen,
    Followed,
}

impl Hold {
    fn locks(self) -> bool {
        matches!(self, Hold::Chosen | Hold::Followed)
    }
}

#[derive(Debug, Default)]
pub(crate) struct ScreenState {
    pressed: BTreeSet<u32>,
    buttons: BTreeSet<u8>,
    hold: Hold,
    viewport: Option<Viewport>,
}

#[derive(rikka_ui::Widget)]
pub(crate) struct Screen {
    #[debug]
    feed: Feed,
    commands: UnboundedSender<Command>,
    display: Option<(u32, u32)>,
    cursor: CursorShape,
    sprite: Option<Sprite>,
    pointer: Option<Point>,
    pointer_mode: PointerMode,
    on_escape: Action<(Escape,)>,
    #[style]
    layout: Style,
}

pub(crate) fn screen(feed: &Feed, commands: UnboundedSender<Command>) -> Screen {
    Screen {
        feed: feed.clone(),
        commands,
        display: None,
        cursor: CursorShape::Default,
        sprite: None,
        pointer: None,
        pointer_mode: PointerMode::Absolute,
        on_escape: Action::default(),
        layout: Style::default(),
    }
}

impl Screen {
    pub(crate) fn display(mut self, display: Option<(u32, u32)>) -> Self {
        self.display = display;
        self
    }

    pub(crate) fn cursor(mut self, cursor: CursorShape) -> Self {
        self.cursor = cursor;
        self
    }

    pub(crate) fn drawn_cursor(mut self, sprite: Option<Sprite>, pointer: Option<Point>) -> Self {
        self.sprite = sprite;
        self.pointer = pointer;
        self
    }

    pub(crate) fn pointer_mode(mut self, mode: PointerMode) -> Self {
        self.pointer_mode = mode;
        self
    }

    pub(crate) fn on_escape<Marker>(
        mut self,
        handler: impl rikka_ui::IntoHandler<(Escape,), (), Marker>,
    ) -> Self {
        self.on_escape = Action::new(handler);
        self
    }

    fn send(&self, command: Command) {
        if self.commands.send(command).is_err() {
            tracing::debug!("the connection already ended");
        }
    }

    fn picture(&self, bounds: Rect) -> Rect {
        let Some((width, height)) = self.feed_size() else {
            return bounds;
        };
        let scale = (bounds.width() / width).min(bounds.height() / height);
        let size = Size::new(width * scale, height * scale);
        Rect::new(
            bounds.x() + (bounds.width() - size.width) / 2.0,
            bounds.y() + (bounds.height() - size.height) / 2.0,
            size.width,
            size.height,
        )
    }

    fn feed_size(&self) -> Option<(f32, f32)> {
        let shown = self
            .display
            .map(|(width, height)| (width as f32, height as f32));
        shown
            .or_else(|| self.feed.size().map(|size| (size.width, size.height)))
            .filter(|(width, height)| *width > 0.0 && *height > 0.0)
    }

    fn position(&self, bounds: Rect, point: Point) -> Position {
        let picture = self.picture(bounds);
        let extent = (POSITION_EXTENT - 1) as f32;
        let along = |offset: f32, length: f32| {
            ((offset / length.max(1.0)).clamp(0.0, 1.0) * extent).round() as u16
        };
        Position {
            x: along(point.x - picture.x(), picture.width()),
            y: along(point.y - picture.y(), picture.height()),
        }
    }

    fn remote_scale(&self, bounds: Rect) -> f32 {
        match self.display {
            Some((width, _)) => width as f32 / self.picture(bounds).width().max(1.0),
            None => 1.0,
        }
    }

    fn paint_sprite(&self, ctx: &mut PaintCtx<'_>, bounds: Rect) {
        let (Some(sprite), Some(pointer)) = (&self.sprite, self.pointer) else {
            return;
        };
        let picture = self.picture(bounds);
        let scale = 1.0 / self.remote_scale(bounds);
        let rect = Rect::new(
            picture.x() + (pointer.x - sprite.hotspot.x) * scale,
            picture.y() + (pointer.y - sprite.hotspot.y) * scale,
            sprite.width as f32 * scale,
            sprite.height as f32 * scale,
        );
        ctx.draw_image(
            rect,
            ImageSource::Rgba {
                width: sprite.width,
                height: sprite.height,
                pixels: Arc::clone(&sprite.pixels),
            },
        );
    }

    fn key(
        &self,
        state: &mut ScreenState,
        ctx: &mut EventCtx<'_>,
        code: u32,
        modifiers: Modifiers,
        pressed: bool,
    ) {
        ctx.set_handled();
        if pressed && modifiers.control && modifiers.alt && modifiers.shift {
            let escape = match code {
                KEY_M => {
                    self.toggle_hold(state, ctx);
                    return;
                }
                KEY_F => Some(Escape::Fullscreen),
                KEY_Q => Some(Escape::Leave),
                KEY_S => Some(Escape::Stats),
                _ => None,
            };
            if let Some(escape) = escape {
                ctx.act(&self.on_escape, (escape,));
                return;
            }
        }
        let Ok(wire) = u16::try_from(code) else {
            return;
        };
        let changed = match pressed {
            true => state.pressed.insert(code),
            false => state.pressed.remove(&code),
        };
        if changed {
            self.send(Command::Input(InputEvent::Key {
                code: wire,
                pressed,
            }));
        }
    }

    fn hold(&self, state: &mut ScreenState, ctx: &mut EventCtx<'_>, hold: Hold) {
        if state.hold == hold {
            return;
        }
        let locked = state.hold.locks();
        state.hold = hold;
        if hold.locks() == locked {
            return;
        }
        match hold.locks() {
            true => ctx.lock_pointer(),
            false => ctx.release_pointer(),
        }
        ctx.request_paint();
        self.send(Command::PointerLock(hold.locks()));
    }

    fn toggle_hold(&self, state: &mut ScreenState, ctx: &mut EventCtx<'_>) {
        let next = match (state.hold.locks(), self.pointer_mode) {
            (false, _) => Hold::Chosen,
            (true, PointerMode::Relative) => Hold::Declined,
            (true, PointerMode::Absolute) => Hold::Free,
        };
        self.hold(state, ctx, next);
    }

    fn follow_mode(&self, state: &mut ScreenState, ctx: &mut EventCtx<'_>) {
        let next = match (self.pointer_mode, state.hold) {
            (PointerMode::Absolute, Hold::Declined | Hold::Followed) => Hold::Free,
            (PointerMode::Relative, Hold::Free) if ctx.focused() => Hold::Followed,
            (_, hold) => hold,
        };
        self.hold(state, ctx, next);
    }

    fn recapture(&self, state: &mut ScreenState, ctx: &mut EventCtx<'_>) {
        ctx.set_handled();
        ctx.request_focus();
        self.hold(state, ctx, Hold::Followed);
    }

    fn draws_sprite(&self, state: &ScreenState) -> bool {
        state.hold == Hold::Chosen && self.pointer_mode == PointerMode::Absolute
    }

    fn release_all(&self, state: &mut ScreenState, ctx: &mut EventCtx<'_>) {
        if state.hold.locks() {
            self.hold(state, ctx, Hold::Free);
        }
        if !state.pressed.is_empty() || !state.buttons.is_empty() {
            state.pressed.clear();
            state.buttons.clear();
            self.send(Command::Input(InputEvent::ReleaseAll));
        }
    }

    fn button(
        &self,
        state: &mut ScreenState,
        ctx: &mut EventCtx<'_>,
        at: Point,
        button: PointerButton,
        pressed: bool,
    ) {
        ctx.set_handled();
        if pressed {
            ctx.request_focus();
            if !state.hold.locks() {
                ctx.capture_pointer();
            }
        }
        if let Some(button) = wire_button(button) {
            let bit = button.to_wire();
            let changed = match pressed {
                true => state.buttons.insert(bit),
                false => state.buttons.remove(&bit),
            };
            if changed {
                self.send(Command::Input(InputEvent::Button {
                    button,
                    pressed,
                    at: self.position(ctx.bounds(), at),
                }));
            }
        }
        if !pressed && state.buttons.is_empty() && !state.hold.locks() {
            ctx.release_pointer();
        }
    }

    fn scroll(&self, ctx: &mut EventCtx<'_>, delta: ScrollDelta, source: ScrollSource) {
        ctx.set_handled();
        let offset = delta.offset();
        let event = match source {
            ScrollSource::Wheel => InputEvent::Wheel {
                dx: (offset.x * VALUE120 / SCROLL_LINE).round() as i32,
                dy: (offset.y * VALUE120 / SCROLL_LINE).round() as i32,
            },
            ScrollSource::Continuous(_) => {
                let scale = self.remote_scale(ctx.bounds());
                InputEvent::Scroll {
                    dx: offset.x * scale,
                    dy: offset.y * scale,
                }
            }
        };
        self.send(Command::Input(event));
    }

    fn dropped(&self, offer: &rikka_ui::DragOffer) {
        for uri in offer.uris() {
            match rikka_open::uri_to_path(&uri) {
                Some(path) => self.send(Command::Upload(path)),
                None => tracing::debug!(uri, "only local files can be sent"),
            }
        }
    }
}

impl Widget for Screen {
    type State = ScreenState;

    fn layout(&self) -> rikka_ui::Layout {
        rikka_ui::Layout::DEFAULT.with(&self.layout)
    }

    fn behavior(&self, _state: &ScreenState) -> Behavior {
        Behavior::FOCUSABLE
            | Behavior::AUTOFOCUS
            | Behavior::TAKES_FOCUS
            | Behavior::TAKES_TAB
            | Behavior::GRABS_KEYS
            | Behavior::CATCHES_DROPS
    }

    fn paint(&self, state: &mut ScreenState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        ctx.draw_quad(Quad::new(bounds).fill(Color::BLACK));
        ctx.draw_image(self.picture(bounds), ImageSource::Feed(self.feed.clone()));
        if self.draws_sprite(state) {
            self.paint_sprite(ctx, bounds);
        }
        let scale = ctx.scale();
        let refresh = RefreshRate::published().unwrap_or(RefreshRate::SIXTY_HERTZ);
        let viewport = Viewport {
            width: (bounds.width() * scale).round().max(1.0) as u32,
            height: (bounds.height() * scale).round().max(1.0) as u32,
            refresh_mhz: refresh.millihertz(),
        };
        if state.viewport != Some(viewport) {
            state.viewport = Some(viewport);
            self.send(Command::Viewport(viewport));
        }
    }

    fn event(&self, state: &mut ScreenState, event: &Event, ctx: &mut EventCtx<'_>) {
        if let Event::FocusLost = event {
            self.release_all(state, ctx);
            return;
        }
        self.follow_mode(state, ctx);
        match *event {
            Event::KeyPressed {
                code, modifiers, ..
            } => {
                self.key(state, ctx, code.evdev(), modifiers, true);
            }
            Event::KeyReleased {
                code, modifiers, ..
            } => {
                self.key(state, ctx, code.evdev(), modifiers, false);
            }
            Event::PointerMoved(at) if !state.hold.locks() => {
                self.send(Command::Motion(self.position(ctx.bounds(), at)));
            }
            Event::PointerTravelled(delta) if state.hold.locks() => {
                let scale = self.remote_scale(ctx.bounds());
                self.send(Command::Input(InputEvent::Relative {
                    dx: delta.x * scale,
                    dy: delta.y * scale,
                }));
            }
            Event::PointerPressed { .. } if state.hold == Hold::Declined => {
                self.recapture(state, ctx)
            }
            Event::PointerPressed {
                position, button, ..
            } => self.button(state, ctx, position, button, true),
            Event::PointerReleased {
                position, button, ..
            } => self.button(state, ctx, position, button, false),
            Event::Scroll { delta, source, .. } => self.scroll(ctx, delta, source),
            Event::DragMoved { ref offer, .. } => {
                if offer.first_of(&[URI_LIST]).is_some() {
                    ctx.accept_drag(Accept::copy(URI_LIST));
                    ctx.set_handled();
                }
            }
            Event::DragDropped { ref offer, .. } => {
                ctx.set_handled();
                self.dropped(offer);
            }
            _ => {}
        }
    }

    fn cursor(&self, state: &ScreenState, _bounds: Rect, _point: Point) -> Option<CursorShape> {
        Some(match state.hold.locks() {
            true => CursorShape::Hidden,
            false => self.cursor,
        })
    }
}

fn wire_button(button: PointerButton) -> Option<Button> {
    match button {
        PointerButton::Primary => Some(Button::Primary),
        PointerButton::Middle => Some(Button::Middle),
        PointerButton::Secondary => Some(Button::Secondary),
        PointerButton::Back => Some(Button::Back),
        PointerButton::Forward => Some(Button::Forward),
        PointerButton::Other(_) => None,
    }
}
