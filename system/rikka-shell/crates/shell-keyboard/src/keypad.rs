use rikka_seam::{Request, SeamClient};
use rikka_ui::State;
use rikka_virtual_input::{Cap, ModifierState};
use shell_kit::CLOSE;

use crate::CAPABILITY;
use crate::board::Board;
use crate::latch::Latches;
use crate::layout::{self, Press};

#[derive(Debug, State)]
pub(super) struct Keypad {
    seam: SeamClient,
    board: Board,
    latches: Latches,
    down: Option<Cap>,
}

impl Keypad {
    #[must_use]
    pub(crate) fn new(seam: SeamClient, board: Board) -> Keypad {
        Keypad {
            seam,
            board,
            latches: Latches::default(),
            down: None,
        }
    }

    #[must_use]
    pub(super) fn latches(&self) -> &Latches {
        &self.latches
    }

    pub(crate) fn pressed(&mut self, row: usize, place: usize) {
        let Some(slot) = layout::slot_at(row, place) else {
            return;
        };
        match slot.press {
            Press::Types(cap) => self.type_down(cap),
            Press::Holds(modifier) => self.latches.press(modifier),
            Press::Locks => self.latches.lock(),
            Press::Hides => self.seam.notify(Request::new(CAPABILITY, CLOSE)),
        }
    }

    pub(crate) fn released(&mut self, row: usize, place: usize) {
        let Some(slot) = layout::slot_at(row, place) else {
            return;
        };
        let Press::Types(cap) = slot.press else {
            return;
        };
        self.type_up(cap);
    }

    pub(crate) fn leave(&mut self) {
        self.release();
        self.latches = Latches::default();
        self.apply();
    }

    fn type_down(&mut self, cap: Cap) {
        self.release();
        self.apply();
        let Some(keyboard) = self.board.keyboard() else {
            return;
        };
        match keyboard.press(cap) {
            Ok(()) => self.down = Some(cap),
            Err(error) => tracing::warn!("failed to type {cap:?}: {error}"),
        }
    }

    fn type_up(&mut self, cap: Cap) {
        if self.down != Some(cap) {
            return;
        }
        self.release();
        if self.latches.spend() {
            self.apply();
        }
    }

    fn release(&mut self) {
        let (Some(cap), Some(keyboard)) = (self.down.take(), self.board.keyboard()) else {
            return;
        };
        if let Err(error) = keyboard.release(cap) {
            tracing::warn!("failed to release {cap:?}: {error}");
        }
    }

    fn apply(&self) {
        let Some(keyboard) = self.board.keyboard() else {
            return;
        };
        let state = ModifierState {
            depressed: self.latches.active_modifiers(),
            caps_lock: self.latches.is_locked(),
        };
        if let Err(error) = keyboard.set_modifiers(state) {
            tracing::warn!(%error, "failed to set modifiers");
        }
    }
}
