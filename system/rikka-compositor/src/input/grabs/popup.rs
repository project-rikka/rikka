use rikka_compositor_protocols::wl_seat::WlSeat;
use rikka_compositor_seat::{
    Button, GrabFocus, KeyboardFocus, Motion, PointerGrab, PointerHandle, SeatId,
};
use rikka_compositor_server::ClientId;
use rikka_compositor_shell::PopupId;
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::Press;

use crate::context::{InputRef, input_ref, seat_parts, windows_ctx, windows_ref};
use crate::context::{SwitcherCtx, WindowsCtx, switcher_ctx};
use crate::state::Compositor;

#[derive(Debug, Default)]
pub(crate) struct PopupChains(Vec<PopupChain>);

#[derive(Debug)]
struct PopupChain {
    seat: SeatId,
    client: ClientId,
    popups: Vec<PopupId>,
    before: Option<KeyboardFocus>,
}

impl PopupChains {
    pub(crate) fn holds(&self, seat: SeatId) -> bool {
        self.0.iter().any(|chain| chain.seat == seat)
    }

    fn of(&self, seat: SeatId) -> Option<&PopupChain> {
        self.0.iter().find(|chain| chain.seat == seat)
    }

    fn of_mut(&mut self, seat: SeatId) -> Option<&mut PopupChain> {
        self.0.iter_mut().find(|chain| chain.seat == seat)
    }

    fn take(&mut self, seat: SeatId) -> Option<PopupChain> {
        let index = self.0.iter().position(|chain| chain.seat == seat)?;
        Some(self.0.swap_remove(index))
    }
}

#[derive(Debug)]
struct PopupGrab {
    seat: SeatId,
}

impl PointerGrab<Compositor> for PopupGrab {
    fn motion(&mut self, pointer: &mut PointerHandle<'_, Compositor>, motion: &Motion) {
        let state = pointer.state();
        let under = motion
            .under
            .filter(|focus| input_ref!(state).within_popup_chain(self.seat, focus.surface));
        pointer.motion(&Motion { under, ..*motion });
    }

    fn button(&mut self, pointer: &mut PointerHandle<'_, Compositor>, button: &Button) {
        let location = pointer.location();
        let state = pointer.state();
        let inside = input_ref!(state)
            .surface_under(location)
            .is_some_and(|(surface, _)| input_ref!(state).within_popup_chain(self.seat, surface));
        if button.press == Press::Pressed && !inside {
            let state = pointer.state();
            windows_ctx!(state).dismiss_popup_chain(self.seat);
            pointer.end_grab();
            return;
        }
        pointer.button(button);
    }

    fn ended(&mut self, state: &mut Compositor) {
        windows_ctx!(state).dismiss_popup_chain(self.seat);
    }
}

impl InputRef<'_> {
    fn within_popup_chain(&self, seat: SeatId, surface: SurfaceId) -> bool {
        self.input.grabs.popups.of(seat).is_some_and(|chain| {
            self.surfaces
                .get(surface)
                .is_some_and(|surface| surface.client() == chain.client)
        })
    }
}

impl SwitcherCtx<'_> {
    pub(crate) fn grab_popup(&mut self, popup: PopupId, seat: &WlSeat, serial: u32) {
        let Some(state) = self.shell.popup(popup) else {
            return;
        };
        let (client, surface) = (state.client(), state.surface());
        let Some(seat) = self
            .seats
            .seat_id(seat)
            .and_then(|wire| self.seats.issuer(wire, serial))
            .filter(|seat| self.input.seats().contains(seat))
        else {
            return self.shell.dismiss_popup(popup);
        };
        let ours = self.seats.get(seat).is_some_and(|seat_state| {
            seat_state.pointer().grab_start(serial).is_some()
                || seat_state.keyboard().focus().is_some_and(|focus| {
                    self.surfaces
                        .get(focus.surface())
                        .is_some_and(|focused| focused.client() == client)
                })
        });
        let chains = &mut self.input.grabs.popups;
        let taken = chains
            .0
            .iter()
            .any(|chain| chain.seat != seat && chain.client == client);
        if !ours || taken {
            return self.shell.dismiss_popup(popup);
        }
        match chains.of_mut(seat) {
            Some(chain) if chain.client == client => chain.popups.push(popup),
            Some(_) => return self.shell.dismiss_popup(popup),
            None => {
                let before = self.keyboard_focus_of(seat);
                self.input.grabs.popups.0.push(PopupChain {
                    seat,
                    client,
                    popups: vec![popup],
                    before,
                });
                seat_parts!(self)
                    .into_seat(seat)
                    .set_pointer_grab(PopupGrab { seat }, GrabFocus::Keep);
            }
        }
        self.focus_popup(seat, surface);
    }

    fn keyboard_focus_of(&self, seat: SeatId) -> Option<KeyboardFocus> {
        if seat == self.input.seat {
            return windows_ref!(self).keyboard_focus();
        }
        self.seats.get(seat)?.keyboard().focus()
    }

    fn focus_popup(&mut self, seat: SeatId, surface: SurfaceId) {
        seat_parts!(self)
            .into_seat(seat)
            .set_keyboard_focus(Some(KeyboardFocus::Surface(surface)));
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn popup_released(&mut self, popup: PopupId) {
        let chains = &mut self.input.grabs.popups;
        let Some(chain) = chains
            .0
            .iter_mut()
            .find(|chain| chain.popups.contains(&popup))
        else {
            return;
        };
        let seat = chain.seat;
        chain.popups.retain(|member| *member != popup);
        if let Some(top) = chain.popups.last().copied() {
            if let Some(surface) = self.shell.popup(top).map(|state| state.surface()) {
                switcher_ctx!(self).focus_popup(seat, surface);
            }
            return;
        }
        let before = self
            .input
            .grabs
            .popups
            .take(seat)
            .and_then(|chain| chain.before);
        let mut seat_mut = seat_parts!(self).into_seat(seat);
        seat_mut.end_pointer_grab();
        seat_mut.set_keyboard_focus(before);
        if seat == self.input.seat {
            self.refresh_activation_states();
        }
    }

    fn dismiss_popup_chain(&mut self, seat: SeatId) {
        let Some(chain) = self.input.grabs.popups.take(seat) else {
            return;
        };
        if let Some(root) = chain.popups.first() {
            self.shell.dismiss_popup(*root);
        }
        seat_parts!(self)
            .into_seat(seat)
            .set_keyboard_focus(chain.before);
        if seat == self.input.seat {
            self.refresh_activation_states();
        }
    }
}
