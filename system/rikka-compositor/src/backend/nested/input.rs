use std::time::Duration;

use rikka_compositor_geometry::{Logical, Physical, Point, Size, Transform};
use rikka_compositor_seat::{Axis, AxisMotion, InputDevice};
use rikka_compositor_types::{AxisSource, Gesture, Press};
use rikka_input::{DeviceEvent, ScrollPhase, ScrollSource, evdev};
use rikka_types::{Gesture as Recognized, GesturePhase, Point as Offset, SCROLL_LINE};

use crate::input::{Moved, PendingEvent};

const KEYBOARD: InputDevice = InputDevice(0);

const DETENT: f64 = 15.0;

#[derive(Debug, Clone, Copy)]
pub(super) struct Shown {
    pub(super) origin: Point<f64, Logical>,
    pub(super) scale: f64,
    pub(super) transform: Transform,
    pub(super) window: Size<f64, Physical>,
    pub(super) host_scale: f64,
}

impl Shown {
    fn logical(self, at: Offset) -> Point<f64, Logical> {
        let across = Point::<f64, Physical>::new(
            f64::from(at.x) * self.host_scale,
            f64::from(at.y) * self.host_scale,
        );
        let upright = across.transformed_in(self.transform.inverse(), &self.window);
        self.origin + upright.to_logical(self.scale)
    }

    fn travel(self, by: Offset) -> Point<f64, Logical> {
        let turned = Point::<f64, Physical>::new(f64::from(by.x), f64::from(by.y))
            .transformed_in(self.transform.inverse(), &Size::new(0.0, 0.0));
        Point::new(turned.x * self.stretch(), turned.y * self.stretch())
    }

    fn stretch(self) -> f64 {
        self.host_scale / self.scale
    }
}

#[derive(Debug)]
pub(super) enum Arrived {
    Pointed(Point<f64, Logical>),
    Event(PendingEvent),
}

#[derive(Debug, Default)]
pub(super) struct HostInput {
    keys: Vec<u32>,
    buttons: Vec<u32>,
    at: Option<Point<f64, Logical>>,
    travels: bool,
    travelled: Option<Moved>,
    turned: f32,
}

impl HostInput {
    pub(super) fn translate(
        &mut self,
        event: DeviceEvent,
        shown: Shown,
        time: Duration,
        mut emit: impl FnMut(Arrived),
    ) {
        match event {
            DeviceEvent::PointerMoved(at) => {
                let to = shown.logical(at);
                self.at = Some(to);
                emit(match self.travels {
                    true => motion(to, self.travelled.take(), time),
                    false => Arrived::Pointed(to),
                });
            }
            DeviceEvent::PointerTravelled(by) => {
                let by = shown.travel(by);
                let before = self.travelled.map_or(Point::zero(), |moved| moved.delta);
                self.travels = true;
                self.travelled = Some(Moved {
                    delta: before + by,
                    unaccelerated: before + by,
                });
            }
            DeviceEvent::PointerPressed { button, .. } => {
                let code = evdev::button_code(button);
                if hold(&mut self.buttons, code, Press::Pressed) {
                    emit(button_event(code, Press::Pressed, time));
                }
            }
            DeviceEvent::PointerReleased { button, .. } => {
                let code = evdev::button_code(button);
                if hold(&mut self.buttons, code, Press::Released) {
                    emit(button_event(code, Press::Released, time));
                }
            }
            DeviceEvent::PointerLeft => {
                for code in self.buttons.drain(..) {
                    emit(button_event(code, Press::Released, time));
                }
            }
            DeviceEvent::Scroll { delta, source, .. } => {
                let mut axis = Axis::new(time);
                let offset = delta.offset();
                let moved = |along: f32| match source {
                    ScrollSource::Wheel => wheel(f64::from(along)),
                    ScrollSource::Continuous(phase) => {
                        finger(f64::from(along) * shown.stretch(), phase)
                    }
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
                if !axis.is_empty() {
                    emit(Arrived::Event(PendingEvent::Axis(axis)));
                }
            }
            DeviceEvent::Gesture { gesture, .. } => {
                if let Some(gesture) = self.gesture(gesture, shown, time) {
                    emit(Arrived::Event(PendingEvent::Gesture(gesture)));
                }
            }
            DeviceEvent::KeyPressed { code, .. } => {
                if hold(&mut self.keys, code.evdev(), Press::Pressed) {
                    emit(key_event(code.evdev(), Press::Pressed, time));
                }
            }
            DeviceEvent::KeyReleased { code, .. } => {
                if hold(&mut self.keys, code.evdev(), Press::Released) {
                    emit(key_event(code.evdev(), Press::Released, time));
                }
            }
            DeviceEvent::Preedit { .. } | DeviceEvent::Commit(_) => {}
        }
    }

    pub(super) fn settle(&mut self, time: Duration, mut emit: impl FnMut(Arrived)) {
        if let (Some(at), Some(travelled)) = (self.at, self.travelled.take()) {
            emit(motion(at, Some(travelled), time));
        }
    }

    pub(super) fn unfocused(&mut self, time: Duration, mut emit: impl FnMut(Arrived)) {
        for code in self.keys.drain(..) {
            emit(key_event(code, Press::Released, time));
        }
    }

    fn gesture(&mut self, gesture: Recognized, shown: Shown, time: Duration) -> Option<Gesture> {
        let ended = |phase: GesturePhase| phase == GesturePhase::Cancelled;
        Some(match gesture {
            Recognized::Swipe {
                phase,
                fingers,
                delta,
            } => match phase {
                GesturePhase::Began => Gesture::SwipeBegin { fingers, time },
                GesturePhase::Moved => Gesture::SwipeUpdate {
                    delta: shown.travel(delta),
                    time,
                },
                phase => Gesture::SwipeEnd {
                    cancelled: ended(phase),
                    time,
                },
            },
            Recognized::Pinch {
                phase,
                fingers,
                delta,
                scale,
                rotation,
            } => match phase {
                GesturePhase::Began => {
                    self.turned = 0.0;
                    Gesture::PinchBegin { fingers, time }
                }
                GesturePhase::Moved => {
                    let turned = std::mem::replace(&mut self.turned, rotation);
                    Gesture::PinchUpdate {
                        delta: shown.travel(delta),
                        scale: f64::from(scale),
                        rotation: f64::from(rotation - turned),
                        time,
                    }
                }
                phase => Gesture::PinchEnd {
                    cancelled: ended(phase),
                    time,
                },
            },
            Recognized::Hold { phase, fingers } => match phase {
                GesturePhase::Began => Gesture::HoldBegin { fingers, time },
                GesturePhase::Moved => return None,
                phase => Gesture::HoldEnd {
                    cancelled: ended(phase),
                    time,
                },
            },
        })
    }
}

fn hold(held: &mut Vec<u32>, code: u32, press: Press) -> bool {
    let was = held.contains(&code);
    held.retain(|kept| *kept != code);
    if press == Press::Pressed {
        held.push(code);
    }
    was != (press == Press::Pressed)
}

fn motion(to: Point<f64, Logical>, moved: Option<Moved>, time: Duration) -> Arrived {
    Arrived::Event(PendingEvent::Motion {
        to,
        moved,
        placed: false,
        time,
    })
}

fn key_event(code: u32, press: Press, time: Duration) -> Arrived {
    Arrived::Event(PendingEvent::Key {
        device: KEYBOARD,
        code,
        press,
        time,
    })
}

fn button_event(code: u32, press: Press, time: Duration) -> Arrived {
    Arrived::Event(PendingEvent::Button { code, press, time })
}

fn wheel(pixels: f64) -> AxisMotion {
    let detents = pixels / f64::from(SCROLL_LINE);
    AxisMotion {
        value: detents * DETENT,
        v120: (detents != 0.0)
            .then_some((detents * f64::from(evdev::VALUE120_PER_DETENT)).round() as i32),
        ..AxisMotion::default()
    }
}

fn finger(pixels: f64, phase: ScrollPhase) -> AxisMotion {
    AxisMotion {
        value: pixels,
        stopped: phase == ScrollPhase::Ended && pixels == 0.0,
        ..AxisMotion::default()
    }
}
