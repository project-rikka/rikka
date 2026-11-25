mod absolute;
mod actions;
mod activity;
mod bindings;
mod caption;
mod caption_touch;
mod deliver;
mod gesture;
mod grabs;
mod idle;
mod ime;
mod inject;
mod keymap;
mod keys;
mod leds;
mod origin;
mod pending;
mod rules;
mod seat_host;
mod seats;
mod tablet;
mod touch;
mod translate;
mod turn;
mod virtual_pointer;
mod warp;
mod wheel;

use rikka_compositor_input::{DeviceSettings, InputEvent, Leds, PointerRestrictions};
use rikka_compositor_seat::SeatId;
use rikka_compositor_swipes::Swiping;

pub(crate) use absolute::place;
pub(crate) use bindings::{Bindings, KeyAction};
pub(crate) use grabs::{GrabRequest, Grabs};
pub(crate) use ime::InputPopups;
pub use inject::Injected;
pub(crate) use keymap::compile as compile_keymap;
pub(crate) use keys::modifiers;
pub(crate) use pending::{AbsoluteSource, Moved, Pending, PendingEvent};
pub(crate) use rules::{
    Acceleration, ClickMethod, CursorRules, MouseRules, MouseScroll, PointerRules, TapButtons,
    TouchpadRules, TouchpadScroll, TouchpadUse,
};
pub(crate) use seats::PERSON_SEAT;
pub(crate) use virtual_pointer::stretch;

use crate::agent::Agent;
use crate::caret::Carets;
use crate::settings::SessionSettings;
use tablet::TabletDevices;
use wheel::Detent;

#[derive(Debug)]
pub(crate) struct Input {
    rules: PointerRules,
    bindings: Bindings,
    pub(crate) seat: SeatId,
    pub(crate) pending: Pending,
    pub(crate) grabs: Grabs,
    pub(crate) restrictions: PointerRestrictions,
    pub(crate) actions: Vec<KeyAction>,
    pub(crate) agent: Agent,
    pub(crate) caret: Carets,
    pub(crate) popups: InputPopups,
    leds: Leds,
    tablets: TabletDevices,
    detent: Detent,
    swiping: Swiping,
}

impl Input {
    pub(crate) fn new(
        rules: PointerRules,
        bindings: Bindings,
        person: SeatId,
        agent: SeatId,
    ) -> Self {
        Self {
            rules,
            bindings,
            seat: person,
            pending: Pending::default(),
            grabs: Grabs::default(),
            restrictions: PointerRestrictions::empty(),
            actions: Vec::new(),
            agent: Agent::new(agent),
            caret: Carets::default(),
            popups: InputPopups::default(),
            leds: Leds::empty(),
            tablets: TabletDevices::default(),
            detent: Detent::default(),
            swiping: Swiping::default(),
        }
    }

    pub(crate) fn rules(&self) -> &PointerRules {
        &self.rules
    }

    pub(crate) fn seats(&self) -> [SeatId; 2] {
        [self.seat, self.agent.seat()]
    }

    pub(crate) fn queue_event(&mut self, event: &InputEvent) {
        if let Some(event) = translate::translate(&mut self.pending, &self.rules, event) {
            self.pending.push(event);
        }
    }

    pub(crate) fn devices(&self) -> DeviceSettings {
        self.rules.devices()
    }

    pub(crate) fn bindings(&self) -> &Bindings {
        &self.bindings
    }

    pub(crate) fn rebind(&mut self, settings: Option<&SessionSettings>, switches_vt: bool) {
        self.bindings = Bindings::read(settings, switches_vt);
        tracing::debug!("keybindings changed");
    }

    pub(crate) fn follow(&mut self, rules: PointerRules) -> bool {
        if rules == self.rules {
            return false;
        }
        self.rules = rules;
        true
    }
}
