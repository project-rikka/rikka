use x11rb_protocol::protocol::xproto::{ClientMessageEvent, EventMask};

use crate::ewmh::state_list;
use crate::fault::Outcome;
use crate::window::{WindowStates, XWindowId};
use crate::wire::{CURRENT_TIME, NONE};
use crate::xwm::Xwm;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IcccmState {
    Withdrawn = 0,
    Normal = 1,
    Iconic = 3,
}

impl Xwm {
    pub(crate) fn write_icccm_state(&mut self, window: XWindowId, state: IcccmState) -> Outcome {
        let property = self.link.atoms.WM_STATE;
        self.link
            .set_words(window.raw(), property, property, &[state as u32, NONE])
    }

    pub(crate) fn change_states(
        &mut self,
        window: XWindowId,
        states: WindowStates,
        on: bool,
    ) -> Outcome<bool> {
        let Some(record) = self.windows.get_mut(&window) else {
            return Ok(false);
        };
        let tracked = &mut record.window;
        let before = tracked.states;
        tracked.states.set(states, on);
        if tracked.states == before {
            return Ok(false);
        }
        let list = state_list(&self.link.atoms, tracked.states);
        let property = self.link.atoms._NET_WM_STATE;
        self.link.set_atoms(window.raw(), property, &list)?;
        Ok(true)
    }

    pub(crate) fn publish_clients(&mut self) -> Outcome {
        let raw = |windows: &[XWindowId]| -> Vec<u32> {
            windows.iter().map(|window| window.raw()).collect()
        };
        let atoms = self.link.atoms;
        let root = self.link.root;
        let clients = raw(self.stacking.clients());
        self.link
            .set_windows(root, atoms._NET_CLIENT_LIST, &clients)?;
        let mut stacking = raw(self.stacking.order());
        stacking.extend(self.dnd.shown_proxy());
        self.link
            .set_windows(root, atoms._NET_CLIENT_LIST_STACKING, &stacking)
    }

    pub(crate) fn publish_active(&mut self, window: Option<XWindowId>) -> Outcome {
        let root = self.link.root;
        let property = self.link.atoms._NET_ACTIVE_WINDOW;
        self.link
            .set_windows(root, property, &[window.map_or(NONE, XWindowId::raw)])
    }

    pub(crate) fn withdraw_focus(&mut self, window: XWindowId) -> Outcome {
        if self.focused == Some(window) {
            self.focused = None;
            self.publish_active(None)?;
        }
        Ok(())
    }

    pub(crate) fn ask_to_take_focus(&mut self, window: XWindowId) -> Outcome {
        let protocol = self.link.atoms.WM_TAKE_FOCUS;
        self.protocol_message(window, protocol)
    }

    pub(crate) fn protocol_message(&mut self, window: XWindowId, protocol: u32) -> Outcome {
        let message = ClientMessageEvent::new(
            32,
            window.raw(),
            self.link.atoms.WM_PROTOCOLS,
            [protocol, CURRENT_TIME, 0, 0, 0],
        );
        self.link
            .send_event(window.raw(), EventMask::NO_EVENT, message)
    }
}
