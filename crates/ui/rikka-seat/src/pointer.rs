use rikka_input::DeviceEvent;
use rikka_input::evdev;
use rikka_types::{CursorShape, Gesture, Modifiers, Point};
use rikka_wayland_client::wl_pointer::{self, ButtonState, WlPointer};
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1;
use rikka_wayland_client::zwp_relative_pointer_v1::{self, ZwpRelativePointerV1};

use crate::cursor::{Cursor, Cursors, Owner};
use crate::gestures::Gestures;
use crate::scroll::AxisFrame;
use crate::seat_event::SeatEvent;

const FRAME_SINCE_VERSION: u32 = 5;

const RELEASE_SINCE_VERSION: u32 = 3;

#[derive(Debug)]
enum Kind {
    Enter,
    Leave,
    Motion,
    Press { button: u32 },
    Release { button: u32 },
    Axis(AxisFrame),
}

#[derive(Debug)]
struct PendingEvent {
    surface: WlSurface,
    position: Point,
    kind: Kind,
}

#[derive(Debug)]
pub(crate) struct Pointer {
    pub(crate) pointer: WlPointer,
    cursor: Cursor,
    pub(crate) gestures: Option<Gestures>,
    pub(crate) relative: Option<ZwpRelativePointerV1>,
    pub(crate) focus: Option<WlSurface>,
    entered: u32,
    position: Point,
    pending: Vec<PendingEvent>,
    scrolling: bool,
    shown: Option<(CursorShape, u32)>,
}

impl Pointer {
    pub(crate) fn new(pointer: WlPointer, cursor: Cursor, gestures: Option<Gestures>) -> Pointer {
        Pointer {
            pointer,
            cursor,
            gestures,
            relative: None,
            focus: None,
            entered: 0,
            position: Point::ZERO,
            pending: Vec::new(),
            scrolling: false,
            shown: None,
        }
    }

    pub(crate) fn release(self) {
        self.cursor.release();
        if let Some(gestures) = self.gestures {
            gestures.release();
        }
        if let Some(relative) = self.relative {
            relative.destroy();
        }
        if self.pointer.version() >= RELEASE_SINCE_VERSION {
            self.pointer.release();
        }
    }

    pub(crate) fn set_relative_motion(
        &mut self,
        manager: &ZwpRelativePointerManagerV1,
        enabled: bool,
    ) {
        match (enabled, self.relative.take()) {
            (true, None) => {
                self.relative = Some(manager.get_relative_pointer(&self.pointer));
            }
            (true, relative) => self.relative = relative,
            (false, Some(relative)) => relative.destroy(),
            (false, None) => {}
        }
    }

    pub(crate) fn relative_motion(
        &self,
        event: zwp_relative_pointer_v1::Event,
        out: &mut Vec<SeatEvent>,
    ) {
        let zwp_relative_pointer_v1::Event::RelativeMotion {
            dx_unaccel,
            dy_unaccel,
            ..
        } = event
        else {
            return;
        };
        if let Some(surface) = self.focus.clone() {
            out.push(SeatEvent::Input {
                surface,
                event: DeviceEvent::PointerTravelled(Point::new(
                    dx_unaccel as f32,
                    dy_unaccel as f32,
                )),
            });
        }
    }

    pub(crate) fn gesture(
        &self,
        recognized: Option<(WlSurface, Gesture)>,
        out: &mut Vec<SeatEvent>,
    ) {
        if let Some((surface, gesture)) = recognized {
            out.push(SeatEvent::Input {
                surface,
                event: DeviceEvent::Gesture {
                    position: self.position,
                    gesture,
                },
            });
        }
    }

    pub(crate) fn show(
        &mut self,
        surface: &WlSurface,
        shape: CursorShape,
        scale: u32,
        cursors: &mut Cursors,
    ) {
        if self.focus.as_ref() != Some(surface) || self.shown == Some((shape, scale)) {
            return;
        }
        self.shown = Some((shape, scale));
        self.cursor.show(
            Owner::Pointer(&self.pointer),
            self.entered,
            shape,
            scale,
            cursors,
        );
    }

    pub(crate) fn handle(
        &mut self,
        event: wl_pointer::Event,
        modifiers: Modifiers,
        out: &mut Vec<SeatEvent>,
    ) -> Option<u32> {
        let mut pressed = None;
        let mut leaving = None;
        let kind = match event {
            wl_pointer::Event::Enter {
                surface,
                surface_x,
                surface_y,
                serial,
            } => {
                self.focus = Some(surface);
                self.entered = serial;
                self.shown = None;
                self.position = surface_point(surface_x, surface_y);
                Kind::Enter
            }
            wl_pointer::Event::Leave { surface, .. } => {
                if self.focus.as_ref() == Some(&surface) {
                    self.focus = None;
                }
                leaving = Some(surface);
                Kind::Leave
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                self.position = surface_point(surface_x, surface_y);
                Kind::Motion
            }
            wl_pointer::Event::Button {
                button,
                state,
                serial,
                ..
            } => match state {
                Ok(ButtonState::Pressed) => {
                    pressed = Some(serial);
                    Kind::Press { button }
                }
                Ok(ButtonState::Released) => Kind::Release { button },
                unknown => {
                    tracing::warn!("unknown pointer button state: {unknown:?}");
                    return None;
                }
            },
            wl_pointer::Event::Axis { axis, value, .. } => {
                Kind::Axis(AxisFrame::on_axis(axis, |scroll| scroll.absolute = value)?)
            }
            wl_pointer::Event::AxisSource { axis_source } => {
                if let Err(unknown) = axis_source {
                    tracing::warn!("unknown pointer axis source: {unknown:?}");
                    return None;
                }
                Kind::Axis(AxisFrame::default())
            }
            wl_pointer::Event::AxisStop { axis, .. } => {
                Kind::Axis(AxisFrame::on_axis(axis, |scroll| scroll.stop = true)?)
            }
            wl_pointer::Event::AxisDiscrete { axis, discrete } => {
                Kind::Axis(AxisFrame::on_axis(axis, |scroll| {
                    scroll.discrete = discrete
                })?)
            }
            wl_pointer::Event::AxisValue120 { axis, value120 } => {
                Kind::Axis(AxisFrame::on_axis(axis, |scroll| {
                    scroll.value120 = value120
                })?)
            }
            wl_pointer::Event::AxisRelativeDirection { axis, direction } => {
                let Ok(direction) = direction else {
                    tracing::warn!("unknown pointer axis direction: {direction:?}");
                    return None;
                };
                Kind::Axis(AxisFrame::on_axis(axis, |scroll| {
                    scroll.direction = Some(direction);
                })?)
            }
            wl_pointer::Event::Frame => {
                self.flush(modifiers, out);
                return None;
            }
            _ => return None,
        };
        let Some(surface) = leaving.or_else(|| self.focus.clone()) else {
            tracing::debug!("pointer event {kind:?} without focused surface");
            return pressed;
        };
        self.queue(PendingEvent {
            surface,
            position: self.position,
            kind,
        });
        if self.pointer.version() < FRAME_SINCE_VERSION {
            self.flush(modifiers, out);
        }
        pressed
    }

    fn queue(&mut self, event: PendingEvent) {
        if let (
            Some(PendingEvent {
                kind: Kind::Axis(last_axis),
                surface,
                ..
            }),
            Kind::Axis(scrolled),
        ) = (self.pending.last_mut(), &event.kind)
            && *surface == event.surface
            && let Some(merged) = last_axis.merge(scrolled)
        {
            *last_axis = merged;
            return;
        }
        self.pending.push(event);
    }

    fn flush(&mut self, modifiers: Modifiers, out: &mut Vec<SeatEvent>) {
        for event in self.pending.drain(..) {
            let PendingEvent {
                surface,
                position,
                kind,
            } = event;
            let event = match kind {
                Kind::Enter | Kind::Motion => DeviceEvent::PointerMoved(position),
                Kind::Leave => DeviceEvent::PointerLeft,
                Kind::Press { button } => DeviceEvent::PointerPressed {
                    position,
                    button: evdev::pointer_button(button),
                    modifiers,
                },
                Kind::Release { button } => DeviceEvent::PointerReleased {
                    position,
                    button: evdev::pointer_button(button),
                    modifiers,
                },
                Kind::Axis(scrolled) => DeviceEvent::Scroll {
                    position,
                    delta: scrolled.delta(),
                    modifiers,
                    source: scrolled.source(&mut self.scrolling),
                },
            };
            out.push(SeatEvent::Input { surface, event });
        }
    }
}

fn surface_point(x: f64, y: f64) -> Point {
    Point::new(x as f32, y as f32)
}
