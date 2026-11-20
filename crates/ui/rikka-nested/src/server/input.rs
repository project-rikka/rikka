use std::time::{Duration, Instant};

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_seat::{
    Axis, AxisMotion, Button, Composition, TouchDown, TouchMotion, TouchUp,
};
use rikka_compositor_types::{AxisSource, Gesture, Press};
use rikka_input::evdev;
use rikka_types::{
    Gesture as UiGesture, GesturePhase, KeyCode, Point as UiPoint, SCROLL_LINE, ScrollDelta,
};
use rikka_ui::{ScrollPhase, ScrollSource};

use super::state::State;
use crate::input::Input;
use crate::view::ViewId;

const WHEEL_STEP: f64 = 10.0;

fn logical(point: UiPoint) -> Point<f64, Logical> {
    Point::new(f64::from(point.x), f64::from(point.y))
}

fn evdev_code(code: KeyCode) -> Option<u32> {
    code.is_known().then(|| code.xkb() - evdev::XKB_OFFSET)
}

fn press(pressed: bool) -> Press {
    match pressed {
        true => Press::Pressed,
        false => Press::Released,
    }
}

impl State {
    pub(crate) fn input(&mut self, view: ViewId, input: Input, at: Instant) {
        if !self.views.records.contains_key(&view) {
            return;
        }
        let time = self.input_time(at);
        match input {
            Input::PointerMoved { at } => {
                self.point_to(view, logical(at), time);
                self.pointer_frame();
            }
            Input::PointerLeft => self.pointer_left(view, time),
            Input::Button {
                at,
                button,
                pressed,
            } => self.button(view, logical(at), evdev::button_code(button), pressed, time),
            Input::Scroll { at, delta, source } => {
                self.scroll(view, logical(at), delta, source, time)
            }
            Input::Gesture { at, gesture } => self.gesture(view, logical(at), gesture, time),
            Input::Travelled { delta } => self.travel(logical(delta), time),
            Input::Key { code, pressed } => {
                if let Some(code) = evdev_code(code) {
                    self.key(view, code, pressed, at);
                }
            }
            Input::Preedit { text, cursor } => {
                let (begin, end) = cursor.map_or((-1, -1), |range| {
                    (
                        i32::try_from(range.start).unwrap_or(-1),
                        i32::try_from(range.end).unwrap_or(-1),
                    )
                });
                self.compose(Composition {
                    preedit: Some((text, begin, end)),
                    ..Composition::default()
                });
            }
            Input::Commit(text) => self.compose(Composition {
                preedit: Some((String::new(), -1, -1)),
                commit: Some(text),
                ..Composition::default()
            }),
            Input::DeleteSurrounding { before, after } => self.compose(Composition {
                delete: Some((before, after)),
                ..Composition::default()
            }),
            Input::Focus(gained) => self.focus(view, gained, at),
            Input::TouchDown { slot, at } => self.touch_down(view, slot, logical(at), time),
            Input::TouchMoved { slot, at } => self.touch_moved(view, slot, logical(at), time),
            Input::TouchUp { slot } => self.touch_up(slot, time),
            Input::TouchCancelled => {
                let seat = self.seat;
                rikka_compositor_seat::seat(self, seat).touch_cancel();
            }
            Input::DragMoved { at, offer } => self.drop_moved(view, logical(at), offer, time),
            Input::DragLeft => self.drop_left(),
            Input::DragDropped { at, offer, .. } => {
                self.drop_moved(view, logical(at), offer, time);
                self.drop_released();
            }
        }
    }

    fn pointer_frame(&mut self) {
        let seat = self.seat;
        rikka_compositor_seat::seat(self, seat).frame();
    }

    fn compose(&mut self, composition: Composition) {
        let seat = self.seat;
        rikka_compositor_seat::seat(self, seat).compose(&composition);
    }

    fn button(
        &mut self,
        view: ViewId,
        at: Point<f64, Logical>,
        code: u32,
        pressed: bool,
        time: Duration,
    ) {
        self.point_to(view, at, time);
        if pressed {
            let inside = self
                .surface_at(view, at)
                .is_some_and(|focus| self.grab_contains(view, focus.surface));
            if !inside {
                self.dismiss_grabs(view);
                self.pointer_frame();
                return;
            }
        }
        let seat = self.seat;
        let mut seat_handle = rikka_compositor_seat::seat(self, seat);
        seat_handle.button(Button {
            code,
            press: press(pressed),
            time,
        });
        seat_handle.frame();
    }

    fn scroll(
        &mut self,
        view: ViewId,
        at: Point<f64, Logical>,
        delta: ScrollDelta,
        source: ScrollSource,
        time: Duration,
    ) {
        self.point_to(view, at, time);
        let mut axis = Axis::new(time);
        let offset = delta.offset();
        let moved = |amount: f32| match source {
            ScrollSource::Wheel => wheel(f64::from(amount) / f64::from(SCROLL_LINE)),
            ScrollSource::Continuous(phase) => AxisMotion {
                value: f64::from(amount),
                stopped: phase == ScrollPhase::Ended,
                ..AxisMotion::default()
            },
        };
        let directed = |motion: AxisMotion| AxisMotion {
            inverted: delta.is_natural(),
            ..motion
        };
        axis.source = Some(match source {
            ScrollSource::Wheel => AxisSource::Wheel,
            ScrollSource::Continuous(_) => AxisSource::Finger,
        });
        axis.horizontal = directed(moved(offset.x));
        axis.vertical = directed(moved(offset.y));
        let seat = self.seat;
        let mut seat_handle = rikka_compositor_seat::seat(self, seat);
        if !axis.is_empty() {
            seat_handle.axis(axis);
        }
        seat_handle.frame();
    }

    fn gesture(
        &mut self,
        view: ViewId,
        at: Point<f64, Logical>,
        gesture: UiGesture,
        time: Duration,
    ) {
        self.point_to(view, at, time);
        let ended = |phase: GesturePhase| phase == GesturePhase::Cancelled;
        let translated = match gesture {
            UiGesture::Swipe {
                phase,
                fingers,
                delta,
            } => match phase {
                GesturePhase::Began => Gesture::SwipeBegin { fingers, time },
                GesturePhase::Moved => Gesture::SwipeUpdate {
                    delta: logical(delta),
                    time,
                },
                GesturePhase::Ended | GesturePhase::Cancelled => Gesture::SwipeEnd {
                    cancelled: ended(phase),
                    time,
                },
            },
            UiGesture::Pinch {
                phase,
                fingers,
                delta,
                scale,
                rotation,
            } => match phase {
                GesturePhase::Began => Gesture::PinchBegin { fingers, time },
                GesturePhase::Moved => Gesture::PinchUpdate {
                    delta: logical(delta),
                    scale: f64::from(scale),
                    rotation: f64::from(rotation),
                    time,
                },
                GesturePhase::Ended | GesturePhase::Cancelled => Gesture::PinchEnd {
                    cancelled: ended(phase),
                    time,
                },
            },
            UiGesture::Hold { phase, fingers } => match phase {
                GesturePhase::Began => Gesture::HoldBegin { fingers, time },
                GesturePhase::Moved => return,
                GesturePhase::Ended | GesturePhase::Cancelled => Gesture::HoldEnd {
                    cancelled: ended(phase),
                    time,
                },
            },
        };
        let seat = self.seat;
        let mut seat_handle = rikka_compositor_seat::seat(self, seat);
        seat_handle.gesture(translated);
        seat_handle.frame();
    }

    fn touch_down(&mut self, view: ViewId, slot: u32, at: Point<f64, Logical>, time: Duration) {
        let under = self.surface_at(view, at);
        let seat = self.seat;
        let mut seat_handle = rikka_compositor_seat::seat(self, seat);
        seat_handle.touch_down(TouchDown {
            slot,
            location: at,
            under,
            time,
        });
        seat_handle.touch_frame();
    }

    fn touch_moved(&mut self, view: ViewId, slot: u32, at: Point<f64, Logical>, time: Duration) {
        let under = self.surface_at(view, at);
        let seat = self.seat;
        let mut seat_handle = rikka_compositor_seat::seat(self, seat);
        seat_handle.touch_motion(TouchMotion {
            slot,
            location: at,
            under,
            time,
        });
        seat_handle.touch_frame();
    }

    fn touch_up(&mut self, slot: u32, time: Duration) {
        let seat = self.seat;
        let mut seat_handle = rikka_compositor_seat::seat(self, seat);
        seat_handle.touch_up(TouchUp { slot, time });
        seat_handle.touch_frame();
    }
}

fn wheel(notches: f64) -> AxisMotion {
    if notches == 0.0 {
        return AxisMotion::default();
    }
    AxisMotion {
        value: notches * WHEEL_STEP,
        v120: Some((notches * 120.0).round() as i32),
        ..AxisMotion::default()
    }
}
