use std::any::Any;
use std::collections::VecDeque;
use std::fmt;

use rikka_compositor_server::DisplayHandle;
use rikka_compositor_surface::{SurfaceHost, Surfaces};

use crate::control::SeatMut;
use crate::event::SeatEvent;
use crate::host::SeatHost;
use crate::ids::SeatId;
use crate::pointer::PointerGrab;
use crate::seats::Seats;
use crate::touch::TouchGrab;

pub trait SeatAccess: sealed::Access {}

impl<T: sealed::Access> SeatAccess for T {}

pub(crate) mod sealed {
    use super::{DisplayHandle, PointerGrab, SeatEvent, SeatHost, Seats, Surfaces, TouchGrab};

    pub trait Access {
        type Host: SeatHost;

        fn seats(&mut self) -> &mut Seats;

        fn surfaces(&mut self) -> &mut Surfaces;

        fn display(&self) -> &DisplayHandle<Self::Host>;

        fn report(&mut self, event: SeatEvent);

        fn pointer_grab_ended(&mut self, grab: Box<dyn PointerGrab<Self::Host>>);

        fn touch_grab_ended(&mut self, grab: Box<dyn TouchGrab<Self::Host>>);
    }
}

pub(crate) use sealed::Access;

impl<S: SeatHost> Access for S {
    type Host = S;

    fn seats(&mut self) -> &mut Seats {
        SeatHost::seats(self)
    }

    fn surfaces(&mut self) -> &mut Surfaces {
        SurfaceHost::surfaces(self)
    }

    fn display(&self) -> &DisplayHandle<S> {
        SeatHost::display(self)
    }

    fn report(&mut self, event: SeatEvent) {
        self.seat_event(event);
    }

    fn pointer_grab_ended(&mut self, mut grab: Box<dyn PointerGrab<S>>) {
        grab.ended(self);
    }

    fn touch_grab_ended(&mut self, mut grab: Box<dyn TouchGrab<S>>) {
        grab.ended(self);
    }
}

pub struct SeatParts<'a, S: SeatHost> {
    seats: &'a mut Seats,
    surfaces: &'a mut Surfaces,
    display: &'a DisplayHandle<S>,
}

impl<'a, S: SeatHost> SeatParts<'a, S> {
    pub fn new(
        seats: &'a mut Seats,
        surfaces: &'a mut Surfaces,
        display: &'a DisplayHandle<S>,
    ) -> Self {
        Self {
            seats,
            surfaces,
            display,
        }
    }

    pub fn seat(&mut self, seat: SeatId) -> SeatMut<'_, Self> {
        crate::control::seat(self, seat)
    }

    pub fn into_seat(self, seat: SeatId) -> SeatMut<'a, Self> {
        SeatMut::owning(self, seat)
    }

    pub fn seats(&mut self) -> &mut Seats {
        self.seats
    }

    pub fn surfaces(&mut self) -> &mut Surfaces {
        self.surfaces
    }

    pub fn display(&self) -> &DisplayHandle<S> {
        self.display
    }
}

impl<S: SeatHost> Access for SeatParts<'_, S> {
    type Host = S;

    fn seats(&mut self) -> &mut Seats {
        self.seats
    }

    fn surfaces(&mut self) -> &mut Surfaces {
        self.surfaces
    }

    fn display(&self) -> &DisplayHandle<S> {
        self.display
    }

    fn report(&mut self, event: SeatEvent) {
        self.seats.pending.push_back(Pending::Report(event));
    }

    fn pointer_grab_ended(&mut self, grab: Box<dyn PointerGrab<S>>) {
        self.seats
            .pending
            .push_back(Pending::PointerGrabEnded(Box::new(grab)));
    }

    fn touch_grab_ended(&mut self, grab: Box<dyn TouchGrab<S>>) {
        self.seats
            .pending
            .push_back(Pending::TouchGrabEnded(Box::new(grab)));
    }
}

impl<S: SeatHost> fmt::Debug for SeatParts<'_, S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SeatParts")
            .field("pending", &self.seats.pending.len())
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
pub(crate) struct Queue(VecDeque<Pending>);

pub(crate) enum Pending {
    Report(SeatEvent),
    PointerGrabEnded(Box<dyn Any>),
    TouchGrabEnded(Box<dyn Any>),
}

impl Queue {
    pub(crate) fn push_back(&mut self, pending: Pending) {
        self.0.push_back(pending);
    }

    pub(crate) fn pop_front(&mut self) -> Option<Pending> {
        self.0.pop_front()
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Debug for Queue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Queue")
            .field("len", &self.0.len())
            .finish()
    }
}

pub fn flush<S: SeatHost>(state: &mut S) -> bool {
    let mut applied = false;
    while let Some(pending) = SeatHost::seats(state).pending.pop_front() {
        applied = true;
        match pending {
            Pending::Report(event) => state.seat_event(event),
            Pending::PointerGrabEnded(grab) => {
                let Ok(mut grab) = grab.downcast::<Box<dyn PointerGrab<S>>>() else {
                    unreachable!("a queued grab is only ever the host's own type");
                };
                grab.ended(state);
            }
            Pending::TouchGrabEnded(grab) => {
                let Ok(mut grab) = grab.downcast::<Box<dyn TouchGrab<S>>>() else {
                    unreachable!("a queued grab is only ever the host's own type");
                };
                grab.ended(state);
            }
        }
    }
    applied
}
