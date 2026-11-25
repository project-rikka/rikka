use std::collections::VecDeque;

use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_wayland_wire::{ClientId, serial_not_before};
use smallvec::SmallVec;

use crate::access::Access;
use crate::event::SeatEvent;
use crate::ids::SeatId;
use crate::seats::Seats;
use crate::{ime, keyboard, pointer};

const ISSUED_SERIALS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Device {
    Pointer,
    Keyboard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Claim {
    Active,
    Passive,
}

#[derive(Debug, Default)]
pub(crate) struct Carrying {
    presented: Vec<Presented>,
    issued: VecDeque<Issued>,
    forgotten: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Presented {
    wire: SeatId,
    client: ClientId,
    device: Device,
    seat: SeatId,
}

#[derive(Debug, Clone, Copy)]
struct Issued {
    wire: SeatId,
    serial: u32,
    seat: SeatId,
}

impl Seats {
    pub fn carrier(&self, seat: SeatId) -> Option<SeatId> {
        self.seats.get(seat)?.carrier
    }

    pub fn issuer(&self, seat: SeatId, serial: u32) -> Option<SeatId> {
        let carrying = &self.carrying;
        if let Some(issued) = carrying
            .issued
            .iter()
            .rev()
            .find(|issued| issued.wire == seat && issued.serial == serial)
        {
            return Some(issued.seat);
        }
        let forgotten = carrying
            .forgotten
            .is_some_and(|forgotten| serial_not_before(forgotten, serial));
        (!forgotten).then_some(seat)
    }

    pub fn keyboard_clients(&self, seat: SeatId) -> SmallVec<[ClientId; 2]> {
        let mut clients = SmallVec::new();
        for sharer in self.sharing(seat) {
            let Some(client) = self
                .seats
                .get(sharer)
                .and_then(|entry| entry.keyboard.target.as_ref())
                .map(WlSurface::client)
            else {
                continue;
            };
            let wire = self.wire(sharer, client);
            if self.presenter(wire, client, Device::Keyboard) == sharer
                && !clients.contains(&client)
            {
                clients.push(client);
            }
        }
        clients
    }

    pub fn wire(&self, seat: SeatId, client: ClientId) -> SeatId {
        match self.seats.get(seat) {
            Some(entry) => match entry.carrier {
                Some(carrier) if !entry.bound_by(client) => carrier,
                _ => seat,
            },
            None => seat,
        }
    }

    pub(crate) fn presenter(&self, wire: SeatId, client: ClientId, device: Device) -> SeatId {
        self.carrying
            .presented
            .iter()
            .find(|presented| {
                presented.wire == wire && presented.client == client && presented.device == device
            })
            .map_or(wire, |presented| presented.seat)
    }

    pub(crate) fn presents(&self, seat: SeatId, client: ClientId, device: Device) -> bool {
        self.presenter(self.wire(seat, client), client, device) == seat
    }

    pub(crate) fn focuses(&self, seat: SeatId, client: ClientId, device: Device) -> bool {
        let Some(entry) = self.seats.get(seat) else {
            return false;
        };
        match device {
            Device::Pointer => entry
                .pointer
                .focused
                .as_ref()
                .is_some_and(|focused| focused.client() == client && focused.surface.alive()),
            Device::Keyboard => {
                !entry.keyboard.withheld
                    && entry
                        .keyboard
                        .target
                        .as_ref()
                        .is_some_and(|target| target.client() == client && target.alive())
            }
        }
    }

    pub(crate) fn carry_serial(&mut self, seat: SeatId, wire: SeatId, serial: u32) {
        if seat == wire {
            return;
        }
        let carrying = &mut self.carrying;
        if carrying.issued.len() == ISSUED_SERIALS {
            carrying.forgotten = carrying.issued.pop_front().map(|issued| issued.serial);
        }
        carrying.issued.push_back(Issued { wire, serial, seat });
    }

    fn sharing(&self, seat: SeatId) -> impl Iterator<Item = SeatId> + '_ {
        std::iter::once(seat).chain(
            self.seats
                .iter()
                .filter(move |(_, entry)| entry.carrier == Some(seat))
                .map(|(id, _)| id),
        )
    }

    fn pins(&self, seat: SeatId, device: Device) -> bool {
        let Some(entry) = self.seats.get(seat) else {
            return false;
        };
        match device {
            Device::Pointer => {
                let pointer = &entry.pointer;
                !pointer.buttons.is_empty()
                    || pointer.gesture.is_some()
                    || pointer.constraint().is_some()
            }
            Device::Keyboard => !entry.keyboard.delivered().is_empty(),
        }
    }

    fn present(&mut self, wire: SeatId, client: ClientId, device: Device, seat: SeatId) {
        let presented = &mut self.carrying.presented;
        presented.retain(|presented| {
            presented.wire != wire || presented.client != client || presented.device != device
        });
        if seat != wire {
            presented.push(Presented {
                wire,
                client,
                device,
                seat,
            });
        }
    }
}

pub(crate) fn enter<A: Access>(
    state: &mut A,
    seat: SeatId,
    client: ClientId,
    device: Device,
    claim: Claim,
) {
    if state.seats().presents(seat, client, device) {
        reveal(state, seat, client, device, seat);
    } else {
        self::claim(state, seat, client, device, claim);
    }
}

pub(crate) fn unwire<A: Access>(state: &mut A, seat: SeatId, client: ClientId) {
    if state.seats().wire(seat, client) == seat {
        return;
    }
    for device in [Device::Pointer, Device::Keyboard] {
        if state.seats().presents(seat, client, device) {
            conceal(state, seat, client, device, seat);
            vacate(state, seat, client, device);
        }
    }
    pointer::deliver_frame(state, seat);
}

pub(crate) fn claim<A: Access>(
    state: &mut A,
    seat: SeatId,
    client: ClientId,
    device: Device,
    claim: Claim,
) -> bool {
    let seats = state.seats();
    let wire = seats.wire(seat, client);
    let current = seats.presenter(wire, client, device);
    if current == seat {
        return true;
    }
    if seats.focuses(current, client, device) {
        if claim == Claim::Passive || seats.pins(current, device) {
            return false;
        }
        conceal(state, current, client, device, seat);
    }
    hand_over(state, wire, client, device, seat);
    reveal(state, seat, client, device, seat);
    true
}

pub(crate) fn vacate<A: Access>(state: &mut A, seat: SeatId, client: ClientId, device: Device) {
    let seats = state.seats();
    let wire = seats.wire(seat, client);
    if seats.presenter(wire, client, device) != seat {
        return;
    }
    let next = seats.sharing(wire).find(|other| {
        *other != seat
            && seats.wire(*other, client) == wire
            && seats.focuses(*other, client, device)
    });
    if next.is_none() && seat == wire {
        return;
    }
    if device == Device::Keyboard {
        ime::leave_focus(state, seat);
    }
    hand_over(state, wire, client, device, next.unwrap_or(wire));
    if let Some(next) = next {
        reveal(state, next, client, device, seat);
    }
}

fn hand_over<A: Access>(
    state: &mut A,
    wire: SeatId,
    client: ClientId,
    device: Device,
    seat: SeatId,
) {
    state.seats().present(wire, client, device, seat);
    if device == Device::Keyboard {
        state.report(SeatEvent::KeyboardPresented { seat: wire });
    }
}

fn reveal<A: Access>(state: &mut A, seat: SeatId, client: ClientId, device: Device, actor: SeatId) {
    match device {
        Device::Pointer => pointer::reveal(state, seat, client, actor),
        Device::Keyboard => {
            keyboard::reveal(state, seat, client);
            ime::enter_focus(state, seat);
        }
    }
}

fn conceal<A: Access>(
    state: &mut A,
    seat: SeatId,
    client: ClientId,
    device: Device,
    actor: SeatId,
) {
    match device {
        Device::Pointer => pointer::conceal(state, seat, client, actor),
        Device::Keyboard => {
            keyboard::conceal(state, seat, client);
            ime::leave_focus(state, seat);
        }
    }
}
