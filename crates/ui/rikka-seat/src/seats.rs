use std::time::Instant;

use rikka_types::{CursorShape, KeyboardState, Rect};
use rikka_wayland::{Display, Recorder, SerialKind, Serials};
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::wl_pointer::WlPointer;
use rikka_wayland_client::wl_registry::WlRegistry;
use rikka_wayland_client::wl_seat::WlSeat;
use rikka_wayland_client::wl_shm::WlShm;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::zwp_pointer_gestures_v1::ZwpPointerGesturesV1;
use rikka_wayland_client::zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1;
use rikka_wayland_client::zwp_tablet_manager_v2::ZwpTabletManagerV2;
use rikka_wayland_client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use rikka_wayland_client::{Queue, Route};

use crate::cursor::{Cursors, Pictures, Source, Themes};
use crate::host::SeatHost;
use crate::keyboard::Keyboard;
use crate::pad::Pad;
use crate::seat::Seat;
use crate::seat_event::SeatEvent;
use crate::tablet::{Tablet, TabletSeat};
use crate::text_input::TextInput;

const SEAT: u32 = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grab {
    pub seat: WlSeat,
    pub serial: u32,
}

#[derive(Debug)]
pub(crate) struct Managers {
    pub(crate) cursors: Cursors,
    pub(crate) gestures: Option<ZwpPointerGesturesV1>,
    pub(crate) relative: Option<ZwpRelativePointerManagerV1>,
    pub(crate) relative_motion_enabled: bool,
    pub(crate) text_inputs: Option<ZwpTextInputManagerV3>,
    pub(crate) tablets: Option<ZwpTabletManagerV2>,
}

#[derive(Debug)]
pub struct Seats {
    pub(crate) seats: Vec<Seat>,
    pub(crate) managers: Managers,
    pub(crate) events: Vec<SeatEvent>,
    pub(crate) grab: Option<Grab>,
    serials: Serials,
    recorder: Recorder,
    route: Route,
}

#[must_use]
#[derive(Clone, Copy, Debug, Default)]
pub struct SeatsBuilder {
    gestures: bool,
    relative: bool,
}

impl SeatsBuilder {
    pub fn gestures(mut self) -> Self {
        self.gestures = true;
        self
    }

    pub fn relative_motion(mut self) -> Self {
        self.relative = true;
        self
    }

    pub fn bind(self, display: &Display, route: &Route) -> Seats {
        let shm: Option<WlShm> = display.bind(1..=1).ok();
        let compositor: Option<WlCompositor> = display.bind(1..=6).ok();
        let source = match display.bind(1..=1) {
            Ok(shapes) => Source::Shapes(shapes),
            Err(missing) => {
                tracing::debug!("falling back to cursor theme: {missing}");
                match (&shm, &compositor) {
                    (Some(shm), Some(compositor)) => {
                        Source::Themed(Box::new(Themes::new(shm.clone(), compositor.clone())))
                    }
                    _ => Source::Absent,
                }
            }
        };
        let pictures = shm
            .zip(compositor)
            .map(|(shm, compositor)| Pictures::new(shm, compositor));
        let cursors = Cursors { source, pictures };
        let optional = |enabled: bool| enabled.then_some(display);
        let (serials, recorder) = Serials::recorded();
        Seats {
            seats: Vec::new(),
            managers: Managers {
                cursors,
                gestures: optional(self.gestures)
                    .and_then(|display| display.bind_optional_on(route, 1..=3)),
                relative: optional(self.relative)
                    .and_then(|display| display.bind_optional_on(route, 1..=1)),
                relative_motion_enabled: false,
                text_inputs: display.bind_optional_on(route, 1..=1),
                tablets: display.bind_optional_on(route, 1..=2),
            },
            events: Vec::new(),
            grab: None,
            serials,
            recorder,
            route: route.clone(),
        }
    }
}

impl Seats {
    pub fn builder() -> SeatsBuilder {
        SeatsBuilder::default()
    }

    pub fn listen<D: SeatHost>(queue: &mut Queue<D>) {
        crate::dispatch::listen(queue);
    }

    pub fn add(&mut self, registry: &WlRegistry, name: u32, version: u32) {
        let seat: WlSeat = registry.via(&self.route).bind(name, version.min(SEAT));
        let text_input = self
            .managers
            .text_inputs
            .as_ref()
            .map(|manager| TextInput::new(manager.get_text_input(&seat)));
        let tablet = self
            .managers
            .tablets
            .as_ref()
            .map(|manager| TabletSeat::new(manager.get_tablet_seat(&seat)));
        if self.serials.seat().is_none() {
            self.recorder.follow(Some(seat.clone()));
        }
        self.seats.push(Seat::new(name, seat, tablet, text_input));
    }

    #[must_use]
    pub fn serials(&self) -> Serials {
        self.serials.clone()
    }

    pub fn remove(&mut self, name: u32) -> bool {
        let Some(at) = self.seats.iter().position(|seat| seat.global == name) else {
            return false;
        };
        let seat = self.seats.remove(at);
        if self
            .grab
            .as_ref()
            .is_some_and(|grab| grab.seat == seat.seat)
        {
            self.grab = None;
        }
        if self.serials.seat().as_ref() == Some(&seat.seat) {
            self.recorder.follow(self.first().cloned());
        }
        seat.release();
        true
    }
    #[must_use]
    pub fn first(&self) -> Option<&WlSeat> {
        self.seats.first().map(|seat| &seat.seat)
    }

    pub fn take_keyboard_change(&mut self) -> Option<KeyboardState> {
        let mut changed = false;
        for keyboard in self
            .seats
            .iter_mut()
            .filter_map(|seat| seat.keyboard.as_mut())
        {
            changed |= keyboard.take_layout_changed();
        }
        changed.then(|| self.keyboard_state())
    }

    fn keyboard_state(&self) -> KeyboardState {
        let mut keyboards = self.seats.iter().filter_map(|seat| seat.keyboard.as_ref());
        let focused = keyboards.clone().find(|keyboard| keyboard.is_focused());
        focused
            .or_else(|| keyboards.next())
            .map(Keyboard::keyboard_state)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn repeat_due(&self) -> Option<Instant> {
        self.seats
            .iter()
            .filter_map(|seat| seat.keyboard.as_ref()?.repeat_due())
            .min()
    }

    pub fn repeat<D: SeatHost>(host: &mut D, now: Instant) {
        let seats = host.seats();
        let mut events = std::mem::take(&mut seats.events);
        for seat in &mut seats.seats {
            if let Some(keyboard) = seat.keyboard.as_mut() {
                keyboard.repeat(now, seat.modifiers, &mut events);
            }
        }
        deliver(host, events);
    }
    pub fn flush<D: SeatHost>(host: &mut D) {
        let seats = host.seats();
        let mut events = std::mem::take(&mut seats.events);
        seats.flush_tablets(&mut events);
        deliver(host, events);
    }

    pub(crate) fn flush_tablets(&mut self, out: &mut Vec<SeatEvent>) {
        for tablet in self
            .seats
            .iter_mut()
            .filter_map(|seat| seat.tablet.as_mut())
        {
            tablet.flush(out);
        }
    }

    pub fn tablets(&self) -> impl Iterator<Item = &Tablet> {
        self.seats
            .iter()
            .filter_map(|seat| seat.tablet.as_ref())
            .flat_map(|tablet| tablet.tablets.iter().map(|(_, info)| info))
    }

    pub fn pads(&self) -> impl Iterator<Item = &Pad> {
        self.seats
            .iter()
            .filter_map(|seat| seat.tablet.as_ref())
            .flat_map(|tablet| tablet.pads.iter().map(|pad| &pad.info))
    }

    pub fn show_cursor(&mut self, surface: &WlSurface, shape: CursorShape, scale: u32) {
        let Seats {
            seats, managers, ..
        } = self;
        for pointer in seats.iter_mut().filter_map(|seat| seat.pointer.as_mut()) {
            pointer.show(surface, shape, scale, &mut managers.cursors);
        }
        for tablet in seats.iter_mut().filter_map(|seat| seat.tablet.as_mut()) {
            tablet.show(surface, shape, scale, &mut managers.cursors);
        }
    }

    pub fn update_text_input_rects(&mut self, mut caret: impl FnMut(&WlSurface) -> Option<Rect>) {
        for text_input in self
            .seats
            .iter_mut()
            .filter_map(|seat| seat.text_input.as_mut())
        {
            let shown = text_input.focus.as_ref().and_then(&mut caret);
            text_input.set_cursor_rect(shown);
        }
    }

    #[must_use]
    pub fn grab(&self) -> Option<&Grab> {
        self.grab.as_ref()
    }

    #[must_use]
    pub fn pointer_over(&self, surface: &WlSurface) -> Option<&WlPointer> {
        self.seats
            .iter()
            .filter_map(|seat| seat.pointer.as_ref())
            .find(|pointer| pointer.focus.as_ref() == Some(surface))
            .map(|pointer| &pointer.pointer)
    }

    pub fn set_relative_motion(&mut self, enabled: bool) {
        let Some(manager) = self.managers.relative.as_ref() else {
            return;
        };
        self.managers.relative_motion_enabled = enabled;
        for pointer in self
            .seats
            .iter_mut()
            .filter_map(|seat| seat.pointer.as_mut())
        {
            pointer.set_relative_motion(manager, enabled);
        }
    }

    pub(crate) fn record(&self, seat: &WlSeat, stroke: Option<(SerialKind, u32)>) {
        if let Some((stroke, serial)) = stroke {
            self.recorder.record(seat, stroke, serial);
        }
    }

    pub(crate) fn pressed(&mut self, seat: &WlSeat, serial: u32) {
        self.grab = Some(Grab {
            seat: seat.clone(),
            serial,
        });
    }
}

pub(crate) fn deliver<D: SeatHost>(host: &mut D, mut events: Vec<SeatEvent>) {
    for event in events.drain(..) {
        host.handle_event(event);
    }
    let seats = host.seats();
    if seats.events.capacity() < events.capacity() {
        seats.events = events;
    }
}
