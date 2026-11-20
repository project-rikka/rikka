use rikka_types::{Gesture, GesturePhase, Point};
use rikka_wayland_client::wl_pointer::WlPointer;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::zwp_pointer_gesture_hold_v1::{self, ZwpPointerGestureHoldV1};
use rikka_wayland_client::zwp_pointer_gesture_pinch_v1::{self, ZwpPointerGesturePinchV1};
use rikka_wayland_client::zwp_pointer_gesture_swipe_v1::{self, ZwpPointerGestureSwipeV1};
use rikka_wayland_client::zwp_pointer_gestures_v1::ZwpPointerGesturesV1;

const HOLD_SINCE_VERSION: u32 = 3;

#[derive(Clone, Debug)]
struct GestureFocus {
    surface: WlSurface,
    fingers: u32,
}

#[derive(Clone, Debug)]
struct PinchState {
    focus: GestureFocus,
    scale: f32,
    rotation: f32,
}

#[derive(Debug)]
pub(crate) struct Gestures {
    pub(crate) swipe: ZwpPointerGestureSwipeV1,
    pub(crate) pinch: ZwpPointerGesturePinchV1,
    pub(crate) hold: Option<ZwpPointerGestureHoldV1>,
    active_swipe: Option<GestureFocus>,
    active_pinch: Option<PinchState>,
    active_hold: Option<GestureFocus>,
}

impl Gestures {
    pub(crate) fn new(manager: &ZwpPointerGesturesV1, pointer: &WlPointer) -> Gestures {
        Gestures {
            swipe: manager.get_swipe_gesture(pointer),
            pinch: manager.get_pinch_gesture(pointer),
            hold: (manager.version() >= HOLD_SINCE_VERSION)
                .then(|| manager.get_hold_gesture(pointer)),
            active_swipe: None,
            active_pinch: None,
            active_hold: None,
        }
    }

    pub(crate) fn release(self) {
        self.swipe.destroy();
        self.pinch.destroy();
        if let Some(hold) = self.hold {
            hold.destroy();
        }
    }

    pub(crate) fn handle_swipe(
        &mut self,
        event: zwp_pointer_gesture_swipe_v1::Event,
    ) -> Option<(WlSurface, Gesture)> {
        match event {
            zwp_pointer_gesture_swipe_v1::Event::Begin {
                surface, fingers, ..
            } => {
                let focus = GestureFocus { surface, fingers };
                self.active_swipe = Some(focus.clone());
                Some(swipe(focus, GesturePhase::Began, Point::ZERO))
            }
            zwp_pointer_gesture_swipe_v1::Event::Update { dx, dy, .. } => Some(swipe(
                self.active_swipe.clone()?,
                GesturePhase::Moved,
                moved(dx, dy),
            )),
            zwp_pointer_gesture_swipe_v1::Event::End { cancelled, .. } => Some(swipe(
                self.active_swipe.take()?,
                ended(cancelled),
                Point::ZERO,
            )),
            _ => None,
        }
    }

    pub(crate) fn handle_pinch(
        &mut self,
        event: zwp_pointer_gesture_pinch_v1::Event,
    ) -> Option<(WlSurface, Gesture)> {
        match event {
            zwp_pointer_gesture_pinch_v1::Event::Begin {
                surface, fingers, ..
            } => {
                let active_pinch = PinchState {
                    focus: GestureFocus { surface, fingers },
                    scale: 1.0,
                    rotation: 0.0,
                };
                self.active_pinch = Some(active_pinch.clone());
                Some(pinch(active_pinch, GesturePhase::Began, Point::ZERO))
            }
            zwp_pointer_gesture_pinch_v1::Event::Update {
                dx,
                dy,
                scale,
                rotation,
                ..
            } => {
                let active_pinch = self.active_pinch.as_mut()?;
                active_pinch.scale = scale as f32;
                active_pinch.rotation += rotation as f32;
                Some(pinch(
                    active_pinch.clone(),
                    GesturePhase::Moved,
                    moved(dx, dy),
                ))
            }
            zwp_pointer_gesture_pinch_v1::Event::End { cancelled, .. } => Some(pinch(
                self.active_pinch.take()?,
                ended(cancelled),
                Point::ZERO,
            )),
            _ => None,
        }
    }

    pub(crate) fn handle_hold(
        &mut self,
        event: zwp_pointer_gesture_hold_v1::Event,
    ) -> Option<(WlSurface, Gesture)> {
        match event {
            zwp_pointer_gesture_hold_v1::Event::Begin {
                surface, fingers, ..
            } => {
                let focus = GestureFocus { surface, fingers };
                self.active_hold = Some(focus.clone());
                Some(hold(focus, GesturePhase::Began))
            }
            zwp_pointer_gesture_hold_v1::Event::End { cancelled, .. } => {
                Some(hold(self.active_hold.take()?, ended(cancelled)))
            }
            _ => None,
        }
    }
}

fn moved(dx: f64, dy: f64) -> Point {
    Point::new(dx as f32, dy as f32)
}

fn ended(cancelled: i32) -> GesturePhase {
    match cancelled != 0 {
        true => GesturePhase::Cancelled,
        false => GesturePhase::Ended,
    }
}

fn swipe(focus: GestureFocus, phase: GesturePhase, delta: Point) -> (WlSurface, Gesture) {
    let fingers = focus.fingers;
    (
        focus.surface,
        Gesture::Swipe {
            phase,
            fingers,
            delta,
        },
    )
}

fn pinch(active_pinch: PinchState, phase: GesturePhase, delta: Point) -> (WlSurface, Gesture) {
    let fingers = active_pinch.focus.fingers;
    (
        active_pinch.focus.surface,
        Gesture::Pinch {
            phase,
            fingers,
            delta,
            scale: active_pinch.scale,
            rotation: active_pinch.rotation,
        },
    )
}

fn hold(focus: GestureFocus, phase: GesturePhase) -> (WlSurface, Gesture) {
    let fingers = focus.fingers;
    (focus.surface, Gesture::Hold { phase, fingers })
}
