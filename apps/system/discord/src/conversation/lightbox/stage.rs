use std::time::Instant;

use rikka_app::prelude::*;
use rikka_ui::{
    Action, DRAG_SLOP, Element, Gesture, GesturePhase, ImageSource, Layout, ScrollSource, motion,
};

use super::face::Face;
use super::framing::Framing;

const PICTURE: &str = "lightbox-picture";
const KEY_PAN: f32 = 64.0;
const MIDDLE: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Turn {
    Back,
    Forward,
}

pub(super) struct Chrome {
    pub(super) top: Element,
    pub(super) middle: Element,
    pub(super) bottom: Element,
    pub(super) gutter: f32,
}

pub(super) struct Stage {
    face: Face,
    under: Option<ImageSource>,
    natural: Option<Size>,
    gutter: f32,
    chrome: Option<[Element; 3]>,
    menu: Box<dyn Fn() -> Vec<MenuItem>>,
    dismiss: Action,
    turn: Action<(Turn,)>,
    zoom: Action<(bool,)>,
    copy: Action,
}

pub(super) fn stage(face: Face, natural: Option<Size>) -> Stage {
    Stage {
        face,
        under: None,
        natural,
        gutter: 0.0,
        chrome: None,
        menu: Box::new(Vec::new),
        dismiss: Action::default(),
        turn: Action::default(),
        zoom: Action::default(),
        copy: Action::default(),
    }
}

impl Stage {
    pub(super) fn with_underlay(mut self, under: Option<ImageSource>) -> Stage {
        self.under = under;
        self
    }

    pub(super) fn chrome(mut self, chrome: Chrome) -> Stage {
        self.gutter = chrome.gutter;
        self.chrome = Some([chrome.top, chrome.middle, chrome.bottom]);
        self
    }

    pub(super) fn menu(mut self, menu: impl Fn() -> Vec<MenuItem> + 'static) -> Stage {
        self.menu = Box::new(menu);
        self
    }

    pub(super) fn on_dismiss(mut self, dismiss: Action) -> Stage {
        self.dismiss = dismiss;
        self
    }

    pub(super) fn on_turn(mut self, turn: Action<(Turn,)>) -> Stage {
        self.turn = turn;
        self
    }

    pub(super) fn on_zoom(mut self, zoom: Action<(bool,)>) -> Stage {
        self.zoom = zoom;
        self
    }

    pub(super) fn on_copy(mut self, copy: Action) -> Stage {
        self.copy = copy;
        self
    }
}

impl View for Stage {
    fn build(mut self) -> Element {
        let chrome = self.chrome.take().map(Vec::from).unwrap_or_default();
        Element::with_children(self, chrome)
    }
}

#[derive(Default)]
pub(super) struct StageState {
    framing: Option<Framing>,
    grab: Option<Grab>,
    pinch: Option<f32>,
    snap: bool,
    started: Option<Instant>,
    menu: ContextMenu,
}

#[derive(Clone, Copy)]
struct Grab {
    from: Point,
    at: Point,
    dragged: bool,
    on_picture: bool,
}

impl StageState {
    fn steady(&self) -> bool {
        self.pinch.is_none() && !self.grab.is_some_and(|grab| grab.dragged)
    }

    fn zoomed(&self) -> Option<bool> {
        self.framing.as_ref().map(Framing::is_zoomed)
    }
}

impl Widget for Stage {
    type State = StageState;

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn behavior(&self, _state: &StageState) -> Behavior {
        Behavior::FOCUSABLE | Behavior::TAKES_FOCUS | Behavior::AUTOFOCUS | Behavior::CLIPS
    }

    fn paint(&self, state: &mut StageState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let room = self.room(bounds, ctx.child_rect(MIDDLE));
        if let Some(target) = settled(state, self.natural, bounds.size, room).map(Framing::picture)
        {
            if std::mem::take(&mut state.snap) {
                ctx.jump(PICTURE, target);
            }
            let curve = match state.steady() {
                true => motion::GLIDE,
                false => motion::DRAG,
            };
            let shown = ctx
                .animate_curve(PICTURE, target, curve)
                .translate(bounds.origin);
            if let Some(under) = &self.under {
                ctx.draw_image(shown, under.clone());
            }
            match &self.face {
                Face::Still(source) => ctx.draw_image(shown, source.clone()),
                Face::Moving(frames) => frames.paint(&mut state.started, shown, ctx),
                Face::Waiting | Face::Failed => {}
            }
        }
        state.menu.paint(ctx);
    }

    fn event(&self, state: &mut StageState, event: &Event, ctx: &mut EventCtx<'_>) {
        if state.menu.event(event, ctx) {
            return;
        }
        let was = state.zoomed();
        self.handle(state, event, ctx);
        if let (Some(was), Some(now)) = (was, state.zoomed())
            && was != now
        {
            ctx.act(&self.zoom, (now,));
        }
    }

    fn cursor(&self, state: &StageState, bounds: Rect, point: Point) -> Option<CursorShape> {
        let framing = state.framing.as_ref()?;
        if !framing.contains(point - bounds.origin) {
            return None;
        }
        Some(match (framing.is_zoomed(), framing.overflows()) {
            (false, _) => CursorShape::ZoomIn,
            (true, true) => CursorShape::Grab,
            (true, false) => CursorShape::ZoomOut,
        })
    }
}

impl Stage {
    fn room(&self, bounds: Rect, middle: Option<Rect>) -> Rect {
        let Some(middle) = middle else {
            return Rect::from_origin_size(Point::ZERO, bounds.size);
        };
        Rect::new(
            middle.x() - bounds.x() + self.gutter,
            middle.y() - bounds.y(),
            (middle.width() - 2.0 * self.gutter).max(0.0),
            middle.height(),
        )
    }

    fn handle(&self, state: &mut StageState, event: &Event, ctx: &mut EventCtx<'_>) {
        if let Event::Command(Command::Copy) = event {
            ctx.act(&self.copy, ());
            ctx.set_handled();
            return;
        }
        let origin = ctx.bounds().origin;
        let Some(framing) = state.framing.as_mut() else {
            if let Event::KeyPressed { key, modifiers, .. } = *event {
                self.pressed_away(key, modifiers, ctx);
            }
            return;
        };
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                state.grab = Some(Grab {
                    from: position,
                    at: position,
                    dragged: false,
                    on_picture: framing.contains(position - origin),
                });
                ctx.request_focus();
                ctx.capture_pointer();
                ctx.set_handled();
            }
            Event::PointerPressed {
                position,
                button: PointerButton::Secondary,
                ..
            } if state.grab.is_none() && framing.contains(position - origin) => {
                state.menu.open((self.menu)(), position, ctx);
            }
            Event::PointerMoved(position) => {
                let Some(grab) = state.grab.as_mut() else {
                    return;
                };
                let travelled = position - grab.from;
                if !grab.dragged && travelled.x.hypot(travelled.y) > DRAG_SLOP {
                    grab.dragged = true;
                }
                if grab.dragged {
                    framing.pan(position - grab.at);
                    ctx.request_paint();
                }
                grab.at = position;
            }
            Event::PointerReleased {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                let Some(grab) = state.grab.take() else {
                    return;
                };
                ctx.release_pointer();
                ctx.set_handled();
                ctx.request_paint();
                let local = position - origin;
                match (grab.dragged, grab.on_picture) {
                    (true, _) => {}
                    (false, true) => framing.toggle(local),
                    (false, false) if !framing.contains(local) => ctx.act(&self.dismiss, ()),
                    (false, false) => {}
                }
            }
            Event::Scroll {
                position,
                delta,
                modifiers,
                source,
            } => {
                match (source, modifiers.control) {
                    (ScrollSource::Wheel, _) | (_, true) => {
                        framing.notches(delta.zoom(), position - origin);
                    }
                    (ScrollSource::Continuous(_), false) => framing.pan(delta.content()),
                }
                ctx.request_paint();
                ctx.set_handled();
            }
            Event::Gesture {
                position,
                gesture:
                    Gesture::Pinch {
                        phase,
                        scale,
                        delta,
                        ..
                    },
            } => {
                match phase {
                    GesturePhase::Began => state.pinch = Some(framing.scale()),
                    GesturePhase::Moved => {
                        if let Some(from) = state.pinch {
                            framing.pinched(from, scale, position - origin);
                            framing.pan(delta);
                        }
                    }
                    GesturePhase::Ended | GesturePhase::Cancelled => state.pinch = None,
                }
                ctx.request_paint();
                ctx.set_handled();
            }
            Event::KeyPressed { key, modifiers, .. } => {
                if zoom_key(framing, key, modifiers) {
                    ctx.request_paint();
                    ctx.set_handled();
                    return;
                }
                if framing.is_zoomed()
                    && framing.overflows()
                    && !modifiers.control
                    && !modifiers.alt
                {
                    let nudge = match key {
                        Key::ArrowLeft => Some(Point::new(KEY_PAN, 0.0)),
                        Key::ArrowRight => Some(Point::new(-KEY_PAN, 0.0)),
                        Key::ArrowUp => Some(Point::new(0.0, KEY_PAN)),
                        Key::ArrowDown => Some(Point::new(0.0, -KEY_PAN)),
                        _ => None,
                    };
                    if let Some(nudge) = nudge {
                        framing.pan(nudge);
                        ctx.request_paint();
                        ctx.set_handled();
                        return;
                    }
                }
                self.pressed_away(key, modifiers, ctx);
            }
            Event::Command(Command::ContextMenu) => {
                let size = ctx.bounds().size;
                let shown = framing
                    .picture()
                    .intersection(&Rect::from_origin_size(Point::ZERO, size));
                state.menu.open_by_keys((self.menu)(), shown, ctx);
            }
            Event::Command(command) => {
                match command {
                    Command::ZoomIn => framing.zoom_in(),
                    Command::ZoomOut => framing.zoom_out(),
                    Command::ZoomReset => framing.fit(),
                    _ => return,
                }
                ctx.request_paint();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn pressed_away(&self, key: Key, modifiers: Modifiers, ctx: &mut EventCtx<'_>) {
        if modifiers.control || modifiers.alt || modifiers.logo {
            return;
        }
        match key {
            Key::Escape => ctx.act(&self.dismiss, ()),
            Key::ArrowLeft => ctx.act(&self.turn, (Turn::Back,)),
            Key::ArrowRight => ctx.act(&self.turn, (Turn::Forward,)),
            _ => return,
        }
        ctx.set_handled();
    }
}

fn zoom_key(framing: &mut Framing, key: Key, modifiers: Modifiers) -> bool {
    if modifiers.control || modifiers.alt || modifiers.logo {
        return false;
    }
    match key {
        Key::Char('+' | '=') => framing.zoom_in(),
        Key::Char('-' | '_') => framing.zoom_out(),
        Key::Char('0') => framing.fit(),
        Key::Char('1') => framing.actual_size(),
        _ => return false,
    }
    true
}

fn settled(
    state: &mut StageState,
    natural: Option<Size>,
    bounds: Size,
    room: Rect,
) -> Option<&Framing> {
    let natural = natural.filter(|natural| !natural.is_empty())?;
    let framing = match state.framing.as_mut() {
        Some(framing) => {
            framing.show(natural);
            framing
        }
        None => {
            state.snap = true;
            state.framing.insert(Framing::new(natural))
        }
    };
    state.snap |= framing.fit_into(bounds, room);
    Some(framing)
}
