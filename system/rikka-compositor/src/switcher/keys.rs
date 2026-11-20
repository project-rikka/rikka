use rikka_compositor_seat::{KeyFilter, Keystroke};
use rikka_compositor_switcher::Step;
use rikka_compositor_types::Press;
use rikka_input::xkb;
use rikka_session::shortcuts::Shortcut;
use rikka_types::{Key, Modifiers};

use super::Phase;
use crate::context::WindowsCtx;
use crate::context::{SwitcherCtx, person, switcher_ctx, switcher_ref};
use crate::input::{KeyAction, modifiers};

impl SwitcherCtx<'_> {
    pub(crate) fn switch_modifiers(&mut self, pressed: Modifiers) {
        match &self.switcher.phase {
            Phase::Choosing(switching) if switching.choosing.modifiers_released(pressed) => {
                let target = self.finish_switch(true, pressed);
                self.queue_switched(target);
            }
            Phase::Lingering(holding) if !holding.held_by(pressed) => {
                self.switcher.phase = Phase::Idle;
                self.queue_switched(None);
            }
            _ => {}
        }
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn begin_switch_from(&mut self, keystroke: &Keystroke, back: bool) -> KeyFilter {
        if keystroke.seat != self.input.seat {
            return KeyFilter::Forward;
        }
        if self.begin_switch(back, modifiers(keystroke.modifiers)) {
            return KeyFilter::Consume;
        }
        person!(self).restore_keyboard();
        KeyFilter::Forward
    }

    pub(crate) fn switch_key(&mut self, keystroke: &Keystroke) -> Option<KeyFilter> {
        if keystroke.seat != self.input.seat {
            return None;
        }
        match (&self.switcher.phase, keystroke.press) {
            (Phase::Idle, _) => None,
            (Phase::Choosing(_), Press::Pressed) => {
                self.choosing_pressed(keystroke);
                Some(KeyFilter::Consume)
            }
            (Phase::Lingering(_), Press::Pressed) => {
                self.switcher.phase = Phase::Idle;
                None
            }
            (Phase::Choosing(_) | Phase::Lingering(_), Press::Released) => {
                switcher_ctx!(self).stop_switch_repeat_of(keystroke.code);
                let held = switcher_ref!(self).pressed_modifiers();
                switcher_ctx!(self).switch_modifiers(held);
                Some(KeyFilter::Forward)
            }
        }
    }

    fn choosing_pressed(&mut self, keystroke: &Keystroke) {
        let pressed = modifiers(keystroke.modifiers);
        let step = match (self.input.bound(keystroke), xkb::key(keystroke.sym)) {
            (Some(KeyAction::Run(Shortcut::SwitchWindows)), _) => Step::Next,
            (Some(KeyAction::Run(Shortcut::SwitchWindowsBack)), _) => Step::Previous,
            (_, Key::Tab) if pressed.shift => Step::Previous,
            (_, Key::Tab | Key::ArrowRight) => Step::Next,
            (_, Key::ArrowLeft) => Step::Previous,
            (_, Key::ArrowUp) => Step::Up,
            (_, Key::ArrowDown) => Step::Down,
            (_, Key::Home) => Step::First,
            (_, Key::End) => Step::Last,
            (_, Key::Enter | Key::Space) => {
                let target = switcher_ctx!(self).finish_switch(true, pressed);
                return switcher_ctx!(self).queue_switched(target);
            }
            (_, Key::Escape) => {
                switcher_ctx!(self).finish_switch(false, pressed);
                return switcher_ctx!(self).queue_switched(None);
            }
            _ => return,
        };
        self.switch_step(step);
        switcher_ctx!(self).repeat_switch_step(keystroke.code, step);
    }
}
