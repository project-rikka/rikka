use std::time::Duration;

use rikka_compositor_desktop::WindowId;
use rikka_compositor_switcher::{Choosing, Finished, Step};
use rikka_session::switcher::{Candidate, Switch};
use rikka_session::toplevels::Toplevel;
use rikka_types::Modifiers;

use super::{Phase, Repeating, Switching};
use crate::context::WindowsCtx;
use crate::context::{
    SwitcherCtx, SwitcherRef, person, switcher_ctx, switcher_ref, windows_ctx, windows_ref,
};
use crate::input::Input;
use crate::input::{KeyAction, modifiers};
use crate::session::SwitcherRequest;
use crate::state::Compositor;
use crate::switcher::Switcher;

const OPENING: Duration = Duration::from_millis(150);

fn candidate(toplevel: Toplevel) -> Candidate {
    Candidate {
        id: toplevel.id,
        app_id: toplevel.app_id,
        title: toplevel.title,
        width: toplevel.bounds.width,
        height: toplevel.bounds.height,
    }
}

impl Switcher {
    pub(crate) fn switching(&self) -> bool {
        matches!(self.phase, Phase::Choosing(_))
    }
}

impl SwitcherCtx<'_> {
    pub(super) fn finish_switch(&mut self, choose: bool, pressed: Modifiers) -> Option<WindowId> {
        let phase = std::mem::take(&mut self.switcher.phase);
        let Phase::Choosing(switching) = phase else {
            self.switcher.phase = phase;
            return None;
        };
        self.stop_switch_repeat();
        if let Some(token) = self.switcher.opening.take() {
            self.core.reactor.remove(token);
        }
        let Finished { chosen, lingering } = switching.choosing.finish(choose, pressed);
        if let Some(holding) = lingering {
            self.switcher.phase = Phase::Lingering(holding);
        }
        self.session.switches.publish(None);
        chosen
    }

    fn pick_switch(&mut self, id: &str) -> Option<bool> {
        let window = windows_ref!(self).window_called(id)?;
        let Phase::Choosing(switching) = &mut self.switcher.phase else {
            return None;
        };
        switching.choosing.pick(window)
    }

    pub(super) fn queue_switched(&mut self, target: Option<WindowId>) {
        self.input.actions.push(KeyAction::Switched(target));
        self.frames.input_arrived();
    }

    pub(super) fn repeat_switch_step(&mut self, code: u32, step: Step) {
        self.stop_switch_repeat();
        let Some(repeat) = self
            .seats
            .get(self.input.seat)
            .map(|seat| seat.keyboard().repeat())
        else {
            return;
        };
        if !step.repeats() || repeat.rate <= 0 || repeat.delay < 0 {
            return;
        }
        let token = self.core.reactor.after(
            Duration::from_millis(repeat.delay.unsigned_abs().into()),
            |compositor: &mut Compositor| windows_ctx!(compositor).switch_repeated(),
        );
        self.switcher.repeating = Some(Repeating {
            code,
            step,
            every: Duration::from_secs(1) / repeat.rate.unsigned_abs(),
            token,
        });
    }

    pub(super) fn stop_switch_repeat_of(&mut self, code: u32) {
        if self
            .switcher
            .repeating
            .as_ref()
            .is_some_and(|repeating| repeating.code == code)
        {
            self.stop_switch_repeat();
        }
    }

    fn stop_switch_repeat(&mut self) {
        if let Some(repeating) = self.switcher.repeating.take() {
            self.core.reactor.remove(repeating.token);
        }
    }

    pub(crate) fn abandon_switch(&mut self) {
        self.finish_switch(false, Modifiers::NONE);
        self.switcher.phase = Phase::Idle;
        person!(self).restore_keyboard();
    }
}

impl Input {
    fn pending_switch(&self) -> Option<WindowId> {
        self.actions.iter().rev().find_map(|action| match action {
            KeyAction::Switched(target) => *target,
            _ => None,
        })
    }
}

impl SwitcherRef<'_> {
    fn switch_candidates(&self, current: Option<WindowId>) -> Vec<WindowId> {
        let desktop = &self.windows.desktop;
        let offered = |window: WindowId| {
            desktop.window(window).is_some_and(|record| {
                !record.unmanaged() && record.surface().is_some() && record.workspace().is_some()
            }) && desktop
                .policy(window)
                .and_then(|policy| policy.foreign)
                .and_then(|foreign| self.shell.published_identifier(foreign))
                .is_some()
        };
        let mut listed: Vec<WindowId> = Vec::new();
        let hidden = current
            .into_iter()
            .chain(self.windows.recency.order())
            .chain(desktop.stacking().rev());
        for window in hidden {
            if offered(window) && !listed.contains(&window) {
                listed.push(window);
            }
        }
        listed
    }

    pub(super) fn pressed_modifiers(&self) -> Modifiers {
        self.seats
            .get(self.input.seat)
            .map_or(Modifiers::NONE, |seat| {
                modifiers(seat.keyboard().held_modifiers())
            })
    }
}

impl WindowsCtx<'_> {
    pub(super) fn begin_switch(&mut self, back: bool, pressed: Modifiers) -> bool {
        if self.windows.desktop.locked() || self.windows.exclusive_layer_surface().is_some() {
            return false;
        }
        let current = self
            .input
            .pending_switch()
            .or_else(|| windows_ref!(self).focused_window());
        let windows = switcher_ref!(self).switch_candidates(current);
        let Some(choosing) = Choosing::begin(windows, current, back, pressed) else {
            return true;
        };
        self.switcher.serial += 1;
        let sticky = choosing.is_sticky();
        self.switcher.phase = Phase::Choosing(Switching {
            serial: self.switcher.serial,
            output: windows_ref!(self).active_output(),
            shown: false,
            choosing,
        });
        person!(self).withhold_keyboard();
        if sticky {
            self.switch_opened();
        } else {
            let token = self
                .core
                .reactor
                .after(OPENING, |compositor: &mut Compositor| {
                    windows_ctx!(compositor).switch_opened()
                });
            self.switcher.opening = Some(token);
        }
        true
    }

    pub(super) fn switch_step(&mut self, step: Step) {
        let Phase::Choosing(switching) = &mut self.switcher.phase else {
            return;
        };
        if switching.choosing.take(step) {
            self.publish_switch();
        }
    }

    pub(crate) fn switched(&mut self, target: Option<WindowId>) {
        if let Some(window) = target
            && self.windows.desktop.window(window).is_some()
        {
            self.reveal_window(window);
        }
        if matches!(self.switcher.phase, Phase::Idle) {
            person!(self).restore_keyboard();
        }
    }

    pub(crate) fn interrupt_switch(&mut self) {
        if self.switcher.switching() {
            let held = switcher_ref!(self).pressed_modifiers();
            let target = switcher_ctx!(self).finish_switch(false, held);
            self.switched(target);
        }
    }

    pub(crate) fn switch_lost(&mut self, window: WindowId) {
        let Phase::Choosing(switching) = &mut self.switcher.phase else {
            return;
        };
        if !switching.choosing.lose(window) {
            return;
        }
        match switching.choosing.is_empty() {
            true => switcher_ctx!(self).abandon_switch(),
            false => self.publish_switch(),
        }
    }

    pub(crate) fn publish_switch(&mut self) {
        let Phase::Choosing(switching) = &self.switcher.phase else {
            return self.session.switches.publish(None);
        };
        if !switching.shown {
            return self.session.switches.publish(None);
        }
        let windows = switching.choosing.windows().to_vec();
        let output = switching
            .output
            .and_then(|output| self.outputs.wayland().name(output))
            .unwrap_or_default()
            .to_owned();
        let described: Vec<(WindowId, Option<Toplevel>)> = windows
            .into_iter()
            .map(|window| (window, self.toplevel(window, None)))
            .collect();
        let Phase::Choosing(switching) = &mut self.switcher.phase else {
            return;
        };
        for (window, _) in described.iter().filter(|(_, toplevel)| toplevel.is_none()) {
            switching.choosing.lose(*window);
        }
        if switching.choosing.is_empty() {
            return switcher_ctx!(self).abandon_switch();
        }
        let switch = Switch {
            serial: switching.serial,
            output,
            windows: described
                .into_iter()
                .filter_map(|(_, toplevel)| toplevel.map(candidate))
                .collect(),
            selected: switching.choosing.selected(),
        };
        self.session.switches.publish(Some(switch));
    }

    pub(crate) fn apply_switcher_request(&mut self, request: SwitcherRequest) {
        let current = matches!(
            &self.switcher.phase,
            Phase::Choosing(switching) if switching.serial == request.serial()
        );
        if !current {
            return;
        }
        match request {
            SwitcherRequest::Select(pick) => {
                if switcher_ctx!(self).pick_switch(&pick.id) == Some(true) {
                    self.publish_switch();
                }
            }
            SwitcherRequest::Choose(pick) => {
                if switcher_ctx!(self).pick_switch(&pick.id).is_some() {
                    let held = switcher_ref!(self).pressed_modifiers();
                    let target = switcher_ctx!(self).finish_switch(true, held);
                    self.switched(target);
                }
            }
            SwitcherRequest::Dismiss(_) => {
                let held = switcher_ref!(self).pressed_modifiers();
                let target = switcher_ctx!(self).finish_switch(false, held);
                self.switched(target);
            }
            SwitcherRequest::Arrange(arrangement) => {
                if let Phase::Choosing(switching) = &mut self.switcher.phase {
                    switching.choosing.arrange(arrangement.rows);
                }
            }
        }
    }

    fn switch_repeated(&mut self) {
        let Some(repeating) = &self.switcher.repeating else {
            return;
        };
        let (step, every) = (repeating.step, repeating.every);
        self.switch_step(step);
        let token = self
            .core
            .reactor
            .after(every, |compositor: &mut Compositor| {
                windows_ctx!(compositor).switch_repeated()
            });
        if let Some(repeating) = &mut self.switcher.repeating {
            repeating.token = token;
        }
    }

    fn switch_opened(&mut self) {
        self.switcher.opening = None;
        if let Phase::Choosing(switching) = &mut self.switcher.phase {
            switching.shown = true;
            self.publish_switch();
        }
    }
}
