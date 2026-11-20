use std::collections::VecDeque;
use std::fmt;

use rikka_compositor_seat::{SeatAccess, SeatHost, SeatParts, Seats};
use rikka_compositor_server::DisplayHandle;
use rikka_compositor_shell::{Shell, ShellHost};
use rikka_compositor_surface::{SurfaceHost, SurfaceId, Surfaces};

use crate::event::{SelectionEvent, SendDrag, SendSelection};
use crate::host::SelectionHost;
use crate::selections::Selections;

pub trait SelectionAccess: sealed::Access {}

impl<T: sealed::Access> SelectionAccess for T {}

pub(crate) mod sealed {
    use super::{
        DisplayHandle, SeatAccess, Seats, SelectionEvent, SelectionHost, Selections, SendDrag,
        SendSelection, Shell, SurfaceId, Surfaces,
    };

    pub trait Access {
        type Host: SelectionHost;
        type Seat: SeatAccess;

        fn selections(&mut self) -> &mut Selections;

        fn seats(&mut self) -> &mut Seats;

        fn surfaces(&mut self) -> &mut Surfaces;

        fn shell(&mut self) -> &mut Shell;

        fn display(&self) -> &DisplayHandle<Self::Host>;

        fn seat_access(&mut self) -> &mut Self::Seat;

        fn report(&mut self, event: SelectionEvent);

        fn send_selection(&mut self, request: SendSelection);

        fn send_drag(&mut self, request: SendDrag);

        fn foreign_drag_target(&mut self, surface: SurfaceId) -> bool;
    }
}

pub(crate) use sealed::Access;

impl<S: SelectionHost> Access for S {
    type Host = S;
    type Seat = S;

    fn selections(&mut self) -> &mut Selections {
        SelectionHost::selections(self)
    }

    fn seats(&mut self) -> &mut Seats {
        SeatHost::seats(self)
    }

    fn surfaces(&mut self) -> &mut Surfaces {
        SurfaceHost::surfaces(self)
    }

    fn shell(&mut self) -> &mut Shell {
        ShellHost::shell(self)
    }

    fn display(&self) -> &DisplayHandle<S> {
        SeatHost::display(self)
    }

    fn seat_access(&mut self) -> &mut S {
        self
    }

    fn report(&mut self, event: SelectionEvent) {
        self.selection_event(event);
    }

    fn send_selection(&mut self, request: SendSelection) {
        SelectionHost::send_selection(self, request);
    }

    fn send_drag(&mut self, request: SendDrag) {
        SelectionHost::send_drag(self, request);
    }

    fn foreign_drag_target(&mut self, surface: SurfaceId) -> bool {
        SelectionHost::foreign_drag_target(self, surface)
    }
}

pub struct SelectionParts<'a, S: SelectionHost> {
    selections: &'a mut Selections,
    shell: &'a mut Shell,
    seat: SeatParts<'a, S>,
    foreign: &'a dyn Fn(SurfaceId) -> bool,
}

impl<'a, S: SelectionHost> SelectionParts<'a, S> {
    pub fn new(
        selections: &'a mut Selections,
        shell: &'a mut Shell,
        seat: SeatParts<'a, S>,
        foreign: &'a dyn Fn(SurfaceId) -> bool,
    ) -> Self {
        Self {
            selections,
            shell,
            seat,
            foreign,
        }
    }
}

impl<'a, S: SelectionHost> Access for SelectionParts<'a, S> {
    type Host = S;
    type Seat = SeatParts<'a, S>;

    fn selections(&mut self) -> &mut Selections {
        self.selections
    }

    fn seats(&mut self) -> &mut Seats {
        self.seat.seats()
    }

    fn surfaces(&mut self) -> &mut Surfaces {
        self.seat.surfaces()
    }

    fn shell(&mut self) -> &mut Shell {
        self.shell
    }

    fn display(&self) -> &DisplayHandle<S> {
        self.seat.display()
    }

    fn seat_access(&mut self) -> &mut SeatParts<'a, S> {
        &mut self.seat
    }

    fn report(&mut self, event: SelectionEvent) {
        self.selections.pending.0.push_back(Pending::Event(event));
    }

    fn send_selection(&mut self, request: SendSelection) {
        self.selections
            .pending
            .0
            .push_back(Pending::SendSelection(request));
    }

    fn send_drag(&mut self, request: SendDrag) {
        self.selections
            .pending
            .0
            .push_back(Pending::SendDrag(request));
    }

    fn foreign_drag_target(&mut self, surface: SurfaceId) -> bool {
        (self.foreign)(surface)
    }
}

impl<S: SelectionHost> fmt::Debug for SelectionParts<'_, S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelectionParts")
            .field("seat", &self.seat)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
pub(crate) struct Queue(VecDeque<Pending>);

#[derive(Debug)]
pub(crate) enum Pending {
    Event(SelectionEvent),
    SendSelection(SendSelection),
    SendDrag(SendDrag),
}

pub fn flush<S: SelectionHost>(state: &mut S) -> bool {
    let mut applied = false;
    while let Some(pending) = SelectionHost::selections(state).pending.0.pop_front() {
        applied = true;
        match pending {
            Pending::Event(event) => state.selection_event(event),
            Pending::SendSelection(request) => SelectionHost::send_selection(state, request),
            Pending::SendDrag(request) => SelectionHost::send_drag(state, request),
        }
    }
    applied
}
