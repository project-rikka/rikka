mod draw;

use std::sync::Arc;

use rikka_input::Chord;
use rikka_types::{CursorShape, DragActions, Point, PointerButton, Rect, Size};
use rikka_ui::{
    Accept, Behavior, DragIcon, DragStart, Element, Event, EventCtx, Layout, PaintCtx, Style,
    Widget,
};

use crate::input::Input;
use crate::look::{Constraint, Cursor, Look};
use crate::view::{View, ViewId};

use draw::{LayerImage, draw_layer};

type PassThrough = Arc<dyn Fn(&Event) -> bool + Send + Sync>;

#[must_use]
#[derive(rikka_ui::Widget)]
pub struct NestedView {
    view: View,
    #[debug]
    reserved: Vec<Chord>,
    yielding: Option<PassThrough>,
    retains_focus: bool,
    #[style]
    style: Style,
}

pub fn nested_view(view: &View) -> NestedView {
    NestedView {
        view: view.clone(),
        reserved: Vec::new(),
        yielding: None,
        retains_focus: false,
        style: Style::default(),
    }
}

impl NestedView {
    pub fn reserve_shortcuts(mut self, chords: impl IntoIterator<Item = Chord>) -> NestedView {
        self.reserved.extend(chords);
        self
    }

    pub fn pass_through(
        mut self,
        yielded: impl Fn(&Event) -> bool + Send + Sync + 'static,
    ) -> NestedView {
        self.yielding = Some(Arc::new(yielded));
        self
    }

    pub fn retain_focus(mut self, retained: bool) -> NestedView {
        self.retains_focus = retained;
        self
    }

    fn reserves(&self, event: &Event) -> bool {
        let (Event::KeyPressed { key, modifiers, .. } | Event::KeyReleased { key, modifiers, .. }) =
            event
        else {
            return false;
        };
        self.reserved
            .iter()
            .any(|chord| chord.matches(*modifiers, *key))
    }

    fn follow_view(&self, state: &mut NestedViewState) {
        let id = self.view.id();
        if state.view != Some(id) {
            *state = NestedViewState {
                view: Some(id),
                ..NestedViewState::default()
            };
        }
    }

    fn release_focus(&self, state: &mut NestedViewState) {
        if state.client_focused && !state.focused && !self.retains_focus {
            state.client_focused = false;
            self.view.deliver(Input::Focus(false));
        }
    }

    fn follow_constraint(&self, state: &mut NestedViewState, look: &Look, ctx: &mut EventCtx<'_>) {
        let locked = look.constraint == Some(Constraint::Locked);
        if locked && !state.locked {
            ctx.lock_pointer();
        } else if !locked && state.locked {
            ctx.release_pointer();
        }
        state.locked = locked;
    }

    fn start_outgoing_drag(
        &self,
        state: &mut NestedViewState,
        look: &Look,
        at: Point,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        if state.pressed.is_empty() || ctx.bounds().contains(at) {
            return false;
        }
        let Some(drag) = look.drag.as_ref() else {
            return false;
        };
        let Some(offer) = drag.offer.clone() else {
            return false;
        };
        let icon = drag.icon.clone();
        let size = icon
            .as_ref()
            .map_or(Size::new(1.0, 1.0), |layer| layer.rect.size);
        let ghost = DragIcon::new(
            size,
            Point::ZERO,
            ctx.theme().clone(),
            move || match &icon {
                Some(layer) => Element::new(LayerImage::new(layer.clone())),
                None => Element::empty(),
            },
        );
        ctx.start_drag(DragStart::new(offer, ghost).actions(DragActions::COPY_OR_MOVE));
        self.view.deliver(Input::PointerLeft);
        for button in std::mem::take(&mut state.pressed) {
            self.view.deliver(Input::Button {
                at: Point::new(-1.0, -1.0),
                button,
                pressed: false,
            });
        }
        true
    }

    fn translate(
        &self,
        state: &mut NestedViewState,
        event: &Event,
        ctx: &mut EventCtx<'_>,
    ) -> Option<Input> {
        let origin = ctx.bounds().origin;
        let local = |at: Point| Point::new(at.x - origin.x, at.y - origin.y);
        Some(match event {
            Event::PointerMoved(at) => {
                state.pointer = Some(local(*at));
                Input::PointerMoved { at: local(*at) }
            }
            Event::PointerLeft | Event::PointerExited => {
                state.pointer = None;
                Input::PointerLeft
            }
            Event::PointerPressed {
                position, button, ..
            } => {
                ctx.capture_pointer();
                if !state.pressed.contains(button) {
                    state.pressed.push(*button);
                }
                Input::Button {
                    at: local(*position),
                    button: *button,
                    pressed: true,
                }
            }
            Event::PointerReleased {
                position, button, ..
            } => {
                state
                    .pressed
                    .retain(|pressed_button| pressed_button != button);
                if state.pressed.is_empty() && !state.locked {
                    ctx.release_pointer();
                }
                Input::Button {
                    at: local(*position),
                    button: *button,
                    pressed: false,
                }
            }
            Event::PointerTravelled(delta) => Input::Travelled { delta: *delta },
            Event::Scroll {
                position,
                delta,
                source,
                ..
            } => Input::Scroll {
                at: local(*position),
                delta: *delta,
                source: *source,
            },
            Event::Gesture { position, gesture } => Input::Gesture {
                at: local(*position),
                gesture: *gesture,
            },
            Event::KeyPressed { code, .. } => Input::Key {
                code: *code,
                pressed: true,
            },
            Event::KeyReleased { code, .. } => Input::Key {
                code: *code,
                pressed: false,
            },
            Event::Preedit { text, cursor } => Input::Preedit {
                text: text.clone(),
                cursor: cursor.clone(),
            },
            Event::Commit(text) => Input::Commit(text.clone()),
            Event::FocusGained => {
                state.focused = true;
                state.client_focused = true;
                Input::Focus(true)
            }
            Event::FocusLost => {
                state.focused = false;
                self.release_focus(state);
                return None;
            }
            Event::DragMoved { position, offer } => {
                if let Some(mime) = offer.mimes().first() {
                    ctx.accept_drag(Accept::copy_or_move(mime.clone(), offer));
                }
                Input::DragMoved {
                    at: local(*position),
                    offer: offer.clone(),
                }
            }
            Event::DragLeft => Input::DragLeft,
            Event::DragDropped {
                position,
                offer,
                action,
            } => Input::DragDropped {
                at: local(*position),
                offer: offer.clone(),
                action: *action,
            },
            _ => return None,
        })
    }
}

#[derive(Debug, Default)]
pub struct NestedViewState {
    view: Option<ViewId>,
    sent: Option<(u32, u32, u64)>,
    watching: bool,
    pressed: Vec<PointerButton>,
    pointer: Option<Point>,
    locked: bool,
    focused: bool,
    client_focused: bool,
}

impl Widget for NestedView {
    type State = NestedViewState;

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn behavior(&self, _state: &NestedViewState) -> Behavior {
        Behavior::FOCUSABLE
            | Behavior::TAKES_TAB
            | Behavior::TAKES_FOCUS
            | Behavior::TRACKS_HOVER
            | Behavior::CATCHES_DROPS
            | Behavior::CLIPS
    }

    fn paint(&self, state: &mut NestedViewState, ctx: &mut PaintCtx<'_>) {
        self.follow_view(state);
        self.release_focus(state);
        let bounds = ctx.bounds();
        if bounds.is_empty() {
            return;
        }
        if !state.watching {
            let handle = ctx.handle();
            self.view.watch(move || handle.repaint());
            state.watching = true;
        }
        let scale = f64::from(ctx.scale());
        let size = (
            bounds.size.width.round().max(1.0) as u32,
            bounds.size.height.round().max(1.0) as u32,
            scale.to_bits(),
        );
        if state.sent != Some(size) {
            state.sent = Some(size);
            self.view.resize(bounds.size, scale);
        }
        ctx.pulse(self.view.pulse());
        let frame = self.view.frame();
        let look = self.view.look();
        ctx.clipped(bounds, |ctx| {
            for layer in &frame.layers {
                draw_layer(ctx, bounds.origin, layer);
            }
            if let (Cursor::Image { layer, hotspot }, Some(pointer)) = (&look.cursor, state.pointer)
            {
                let at = Point::new(
                    bounds.origin.x + pointer.x - hotspot.x,
                    bounds.origin.y + pointer.y - hotspot.y,
                );
                draw_layer(ctx, at, layer);
            }
        });
    }

    fn event(&self, state: &mut NestedViewState, event: &Event, ctx: &mut EventCtx<'_>) {
        self.follow_view(state);
        if self.yielding.as_ref().is_some_and(|yielded| yielded(event)) || self.reserves(event) {
            return;
        }
        let look = self.view.look();
        self.follow_constraint(state, &look, ctx);
        if let Event::PointerMoved(at) = event
            && self.start_outgoing_drag(state, &look, *at, ctx)
        {
            ctx.set_handled();
            return;
        }
        let Some(input) = self.translate(state, event, ctx) else {
            return;
        };
        if matches!(look.cursor, Cursor::Image { .. }) {
            ctx.request_paint();
        }
        ctx.set_handled();
        self.view.deliver(input);
    }

    fn cursor(
        &self,
        _state: &NestedViewState,
        _bounds: Rect,
        _point: Point,
    ) -> Option<CursorShape> {
        Some(match self.view.look().cursor {
            Cursor::Default => CursorShape::Default,
            Cursor::Shape(shape) => shape,
            Cursor::Hidden | Cursor::Image { .. } => CursorShape::Hidden,
        })
    }

    fn caret(&self, _state: &NestedViewState) -> Option<Rect> {
        let look = self.view.look();
        look.text_input.as_ref()?;
        Some(look.caret.unwrap_or(Rect::new(0.0, 0.0, 1.0, 1.0)))
    }
}
