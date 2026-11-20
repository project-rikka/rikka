use rikka_compositor_protocols::wl_seat::WlSeat;
use rikka_compositor_seat::{
    Button, GrabFocus, KeyboardFocus, Motion, PointerGrab, PointerHandle, SeatId,
};
use rikka_compositor_server::ClientId;
use rikka_compositor_shell::PopupId;
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::Press;

use crate::context::{InputRef, input_ref, person, windows_ctx, windows_ref};
use crate::context::{SwitcherCtx, WindowsCtx, switcher_ctx};
use crate::state::Compositor;

#[derive(Debug)]
pub(crate) struct PopupChain {
    client: ClientId,
    popups: Vec<PopupId>,
    before: Option<KeyboardFocus>,
}

#[derive(Debug)]
struct PopupGrab;

impl PointerGrab<Compositor> for PopupGrab {
    fn motion(&mut self, pointer: &mut PointerHandle<'_, Compositor>, motion: &Motion) {
        let state = pointer.state();
        let under = motion
            .under
            .filter(|focus| input_ref!(state).within_popup_chain(focus.surface));
        pointer.motion(&Motion { under, ..*motion });
    }

    fn button(&mut self, pointer: &mut PointerHandle<'_, Compositor>, button: &Button) {
        let location = pointer.location();
        let state = pointer.state();
        let inside = input_ref!(state)
            .surface_under(location)
            .is_some_and(|(surface, _)| input_ref!(state).within_popup_chain(surface));
        if button.press == Press::Pressed && !inside {
            let state = pointer.state();
            windows_ctx!(state).dismiss_popup_chain();
            pointer.end_grab();
            return;
        }
        pointer.button(button);
    }

    fn ended(&mut self, state: &mut Compositor) {
        windows_ctx!(state).dismiss_popup_chain();
    }
}

impl InputRef<'_> {
    fn within_popup_chain(&self, surface: SurfaceId) -> bool {
        self.input.grabs.popups.as_ref().is_some_and(|chain| {
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
        let ours = self.seats.get(self.input.seat).is_some_and(|seat| {
            seat.pointer().grab_start(serial).is_some()
                || seat.keyboard().focus().is_some_and(|focus| {
                    self.surfaces
                        .get(focus.surface())
                        .is_some_and(|focused| focused.client() == client)
                })
        });
        if !ours || seat.key::<SeatId>() != Some(self.input.seat) {
            return self.shell.dismiss_popup(popup);
        }
        match &mut self.input.grabs.popups {
            Some(chain) if chain.client == client => chain.popups.push(popup),
            Some(_) => return self.shell.dismiss_popup(popup),
            None => {
                self.input.grabs.popups = Some(PopupChain {
                    client,
                    popups: vec![popup],
                    before: windows_ref!(self).keyboard_focus(),
                });
                person!(self).set_pointer_grab(PopupGrab, GrabFocus::Keep);
            }
        }
        self.focus_popup(surface);
    }

    fn focus_popup(&mut self, surface: SurfaceId) {
        person!(self).set_keyboard_focus(Some(KeyboardFocus::Surface(surface)));
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn popup_released(&mut self, popup: PopupId) {
        let Some(chain) = &mut self.input.grabs.popups else {
            return;
        };
        chain.popups.retain(|member| *member != popup);
        if let Some(top) = chain.popups.last().copied() {
            if let Some(surface) = self.shell.popup(top).map(|state| state.surface()) {
                switcher_ctx!(self).focus_popup(surface);
            }
            return;
        }
        let before = self
            .input
            .grabs
            .popups
            .take()
            .and_then(|chain| chain.before);
        person!(self).end_pointer_grab();
        person!(self).set_keyboard_focus(before);
        self.refresh_activation_states();
    }

    fn dismiss_popup_chain(&mut self) {
        let Some(chain) = self.input.grabs.popups.take() else {
            return;
        };
        if let Some(root) = chain.popups.first() {
            self.shell.dismiss_popup(*root);
        }
        person!(self).set_keyboard_focus(chain.before);
        self.refresh_activation_states();
    }
}
