use rikka_compositor_protocols::wl_seat::WlSeat;
use slotmap::SlotMap;

use rikka_compositor_surface::SurfaceId;

use crate::access::Queue;
use crate::carry::Carrying;
use crate::idle::Idle;
use crate::ids::{SeatId, VirtualKeyboardId, VirtualPointerId};
use crate::seat::Seat;
use crate::virtual_keyboard::VirtualKeyboard;
use crate::virtual_pointer::VirtualPointer;

#[derive(Debug, Default)]
pub struct Seats {
    pub(crate) seats: SlotMap<SeatId, Seat>,
    pub(crate) primary: Option<SeatId>,
    pub(crate) carrying: Carrying,
    pub(crate) virtual_keyboards: SlotMap<VirtualKeyboardId, VirtualKeyboard>,
    pub(crate) virtual_pointers: SlotMap<VirtualPointerId, VirtualPointer>,
    pub(crate) idle: Idle,
    pub(crate) pending: Queue,
}

impl Seats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, seat: SeatId) -> Option<&Seat> {
        self.seats.get(seat)
    }

    pub fn iter(&self) -> impl Iterator<Item = (SeatId, &Seat)> {
        self.seats.iter()
    }

    pub fn named(&self, name: &str) -> Option<SeatId> {
        self.seats
            .iter()
            .find_map(|(id, seat)| (seat.name == name).then_some(id))
    }

    pub fn primary(&self) -> Option<SeatId> {
        self.primary
    }

    pub fn seat_id(&self, handle: &WlSeat) -> Option<SeatId> {
        handle
            .key::<SeatId>()
            .filter(|seat| self.seats.contains_key(*seat))
    }

    pub fn idle_inhibitors(&self) -> impl Iterator<Item = SurfaceId> + '_ {
        self.idle.inhibitors()
    }

    pub fn idle_inhibited(&self) -> bool {
        self.idle.inhibited
    }

    pub(crate) fn seat_mut(&mut self, seat: SeatId) -> Option<&mut Seat> {
        self.seats.get_mut(seat)
    }
}
