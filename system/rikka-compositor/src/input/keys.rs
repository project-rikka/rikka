use rikka_compositor_seat::{KeyFilter, Keystroke, Modifiers as SeatModifiers};
use rikka_compositor_types::Press;
use rikka_input::{Chord, xkb};
use rikka_session::shortcuts::{Group, Shortcut};
use rikka_types::Modifiers;

use super::KeyAction;
use crate::context::person;
use crate::context::{WindowsCtx, switcher_ctx};
use crate::input::Input;

pub(crate) fn modifiers(state: SeatModifiers) -> Modifiers {
    [
        (state.ctrl, Modifiers::CONTROL),
        (state.alt, Modifiers::ALT),
        (state.shift, Modifiers::SHIFT),
        (state.logo, Modifiers::LOGO),
    ]
    .into_iter()
    .filter(|(down, _)| *down)
    .fold(Modifiers::NONE, |all, (_, one)| all | one)
}

impl Input {
    pub(crate) fn bound(&self, keystroke: &Keystroke) -> Option<KeyAction> {
        let active = modifiers(keystroke.modifiers);
        let pressed = xkb::key(keystroke.sym);
        let bindings = self.bindings();
        bindings.resolve(active, pressed).or_else(|| {
            let printed = xkb::key(keystroke.latin?);
            (printed != pressed)
                .then(|| bindings.resolve(active, printed))
                .flatten()
        })
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn filter(&mut self, keystroke: &Keystroke) -> KeyFilter {
        if self.windows.desktop.locked() {
            switcher_ctx!(self).abandon_switch();
            return KeyFilter::Forward;
        }
        if let Some(filtered) = self.switch_key(keystroke) {
            return filtered;
        }
        if keystroke.press != Press::Pressed {
            return KeyFilter::Forward;
        }
        let action = match keystroke.shortcuts_inhibited {
            true => None,
            false => self.input.bound(keystroke),
        };
        match action {
            Some(KeyAction::Run(Shortcut::SwitchWindows)) => {
                return self.begin_switch_from(keystroke, false);
            }
            Some(KeyAction::Run(Shortcut::SwitchWindowsBack)) => {
                return self.begin_switch_from(keystroke, true);
            }
            _ if keystroke.seat == self.input.seat => person!(self).restore_keyboard(),
            _ => {}
        }
        if keystroke.shortcuts_inhibited {
            return KeyFilter::Forward;
        }
        let pressed = xkb::key(keystroke.sym);
        if self.snap_escaped(pressed) {
            return KeyFilter::Consume;
        }
        let Some(action) = action else {
            return KeyFilter::Forward;
        };
        if let KeyAction::Run(shortcut) = action
            && shortcut.group() == Group::Columns
            && !self.windows.scrolling_mode()
        {
            return KeyFilter::Forward;
        }
        let active = modifiers(keystroke.modifiers);
        tracing::debug!("{} runs {action:?}", Chord::new(active, pressed));
        self.input.actions.push(action);
        self.frames.input_arrived();
        KeyFilter::Consume
    }
}
