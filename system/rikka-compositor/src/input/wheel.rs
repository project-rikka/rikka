use rikka_compositor_seat::Axis;
use rikka_session::workspaces::COUNT;
use rikka_types::Modifiers;

use super::keys::modifiers;
use crate::context::WindowsCtx;
use crate::context::windows_ref;

const NOTCH: i32 = 120;

const MODIFIER: Modifiers = Modifiers::LOGO;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Detent(i32);

impl Detent {
    fn turn(&mut self, v120: i32) -> i32 {
        if self.0.signum() != v120.signum() {
            self.0 = 0;
        }
        self.0 += v120;
        let steps = self.0 / NOTCH;
        self.0 -= steps * NOTCH;
        steps
    }
}

impl WindowsCtx<'_> {
    pub(super) fn wheel_turns_workspaces(&mut self, axis: &Axis) -> bool {
        if self.core.profile.is_simple() {
            return false;
        }
        let pressed = self
            .seats
            .get(self.input.seat)
            .map(|seat| modifiers(seat.keyboard().modifiers()));
        if self.windows.desktop.locked() || pressed != Some(MODIFIER) {
            self.input.detent = Detent::default();
            return false;
        }
        let Some(v120) = axis.vertical.v120.filter(|notches| *notches != 0) else {
            return true;
        };
        let steps = self.input.detent.turn(v120);
        for _ in 0..steps.unsigned_abs() {
            self.cycle_workspace(steps.signum());
        }
        true
    }

    fn cycle_workspace(&mut self, step: i32) {
        let Some(output) = windows_ref!(self).active_output() else {
            return;
        };
        let Some(showing) = self.windows.showing(output) else {
            return;
        };
        let count = COUNT as i32;
        let number = (showing.number() as i32 - 1 + step).rem_euclid(count) + 1;
        self.show_workspace_on(output, number as usize);
    }
}
