use std::sync::{Arc, MutexGuard};

use rikka_app::prelude::{
    Behavior, Event, EventCtx, Key, Modifiers, PaintCtx, PointerButton, Widget,
};
use rikka_types::Point;
use viewer_media::Step;

use crate::screen::Screen;
use crate::screen::figure::scene::{self, Scene};
use crate::viewer::Viewer;

const TURN_PER_KEY: f32 = 24.0;
const NOTCH_PER_KEY: f32 = 1.0;

#[derive(Debug)]
pub(crate) struct Grip {
    scene: scene::Shared,
}

#[derive(Debug, Default)]
pub(crate) struct GripState {
    grab: Option<(Point, DragMode)>,
    height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragMode {
    Orbit,
    Pan,
}

impl Grip {
    pub(crate) fn new(scene: &scene::Shared) -> Self {
        Self {
            scene: Arc::clone(scene),
        }
    }

    fn scene(&self) -> MutexGuard<'_, Scene> {
        self.scene.lock()
    }

    fn moved(&self, hold: DragMode, x: f32, y: f32, height: f32) {
        let mut scene = self.scene();
        match hold {
            DragMode::Orbit => scene.turn(x, y),
            DragMode::Pan => scene.pan(x, y, height),
        }
    }

    fn pressed(&self, key: Key, ctx: &mut EventCtx<'_>) {
        if handled_shortcut(key, ctx) {
            return;
        }
        let mut scene = self.scene();
        match key {
            Key::ArrowLeft => scene.turn(-TURN_PER_KEY, 0.0),
            Key::ArrowRight => scene.turn(TURN_PER_KEY, 0.0),
            Key::ArrowUp => scene.turn(0.0, -TURN_PER_KEY),
            Key::ArrowDown => scene.turn(0.0, TURN_PER_KEY),
            Key::Char('+' | '=') => scene.dolly(NOTCH_PER_KEY),
            Key::Char('-' | '_') => scene.dolly(-NOTCH_PER_KEY),
            Key::Char('0' | 'f' | 'F') => scene.recenter(),
            _ => return,
        }
        ctx.request_paint();
    }
}

impl Widget for Grip {
    type State = GripState;

    fn paint(&self, state: &mut GripState, ctx: &mut PaintCtx<'_>) {
        state.height = ctx.bounds().height();
    }

    fn event(&self, state: &mut GripState, event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerPressed {
                position,
                button,
                modifiers,
            } => {
                let Some(hold) = hold(button, modifiers) else {
                    return;
                };
                ctx.request_focus();
                state.grab = Some((position, hold));
                ctx.capture_pointer();
            }
            Event::PointerMoved(position) => {
                let Some((before, hold)) = state.grab else {
                    return;
                };
                state.grab = Some((position, hold));
                self.moved(
                    hold,
                    position.x - before.x,
                    position.y - before.y,
                    state.height,
                );
                ctx.request_paint();
            }
            Event::PointerReleased { .. } => {
                state.grab = None;
                ctx.release_pointer();
            }
            Event::Scroll { delta, .. } => {
                self.scene().dolly(delta.zoom());
                ctx.request_paint();
            }
            Event::KeyPressed { key, .. } => self.pressed(key, ctx),
            _ => {}
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::FOCUSABLE | Behavior::AUTOFOCUS
    }
}

fn hold(button: PointerButton, modifiers: Modifiers) -> Option<DragMode> {
    match button {
        PointerButton::Primary if modifiers.shift => Some(DragMode::Pan),
        PointerButton::Primary => Some(DragMode::Orbit),
        PointerButton::Middle => Some(DragMode::Pan),
        _ => None,
    }
}

fn handled_shortcut(key: Key, ctx: &mut EventCtx<'_>) -> bool {
    match key {
        Key::Space | Key::Enter => ctx.run(|screen: &mut Screen| screen.play_pause()),
        Key::PageUp => ctx.run(|viewer: &mut Viewer| viewer.step(Step::Previous)),
        Key::PageDown => ctx.run(|viewer: &mut Viewer| viewer.step(Step::Next)),
        _ => return false,
    };
    true
}
