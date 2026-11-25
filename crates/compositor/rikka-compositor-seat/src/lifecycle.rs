use rikka_compositor_protocols::wl_seat::WlSeat;
use rikka_compositor_server::{Client, DisplayHandle};
use rikka_compositor_surface::{Committed, SurfaceId};
use smallvec::SmallVec;

use crate::access::Access;
use crate::idle;
use crate::ids::SeatId;
use crate::ime::{self, Ime};
use crate::keyboard::{self, Keyboard, Keymap};
use crate::pointer::{self, Pointer};
use crate::seat::{Capabilities, Seat};
use crate::seats::Seats;
use crate::tablet::{self, Tablets};
use crate::touch::{self, Touch};

impl Seats {
    pub fn add<A: Access>(
        &mut self,
        display: &DisplayHandle<A>,
        name: &str,
        keymap: Keymap,
    ) -> SeatId {
        let id = self.insert(name, keymap, None);
        display.global::<WlSeat>().key(id).publish();
        self.primary.get_or_insert(id);
        id
    }

    pub fn add_carried<A: Access>(
        &mut self,
        display: &DisplayHandle<A>,
        name: &str,
        keymap: Keymap,
        carrier: SeatId,
        visible: impl Fn(&Client) -> bool + 'static,
    ) -> SeatId {
        let id = self.insert(name, keymap, Some(carrier));
        display.global::<WlSeat>().key(id).only(visible).publish();
        id
    }

    fn insert(&mut self, name: &str, keymap: Keymap, carrier: Option<SeatId>) -> SeatId {
        self.seats.insert(Seat {
            name: name.to_owned(),
            capabilities: Capabilities::POINTER | Capabilities::KEYBOARD,
            carrier,
            handles: Vec::new(),
            keyboard: Keyboard::new(keymap),
            pointer: Pointer::new(),
            touch: Touch::default(),
            tablets: Tablets::default(),
            ime: Ime::default(),
        })
    }
}

pub fn surface_committed<A: Access>(state: &mut A, commit: &Committed) {
    let constrained: SmallVec<[SeatId; 2]> = state
        .seats()
        .seats
        .iter()
        .filter(|(_, seat)| seat.pointer.constrains(commit.surface))
        .map(|(id, _)| id)
        .collect();
    for seat in constrained {
        pointer::constraints_committed(state, seat, commit);
    }
}

pub fn surface_destroyed<A: Access>(state: &mut A, surface: SurfaceId) {
    let seats: SmallVec<[SeatId; 2]> = state.seats().seats.keys().collect();
    for seat in seats {
        keyboard::refocus_after_destroy(state, seat, surface);
        pointer::constraints_surface_gone(state, seat, surface);
        pointer::pointer_surface_gone(state, seat, surface);
        pointer::cursor_surface_gone(state, seat, surface);
        touch::touch_surface_gone(state, seat, surface);
        tablet::tools_surface_gone(state, seat, surface);
        tablet::pads_surface_gone(state, seat, surface);
        ime::popups_surface_gone(state, seat, surface);
    }
    idle::inhibitors_surface_gone(state, surface);
}
