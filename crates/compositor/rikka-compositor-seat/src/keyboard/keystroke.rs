use std::time::Duration;

use rikka_compositor_types::Press;
use xkbcommon::xkb::{self, Keycode, Keysym};

use crate::ids::SeatId;
use crate::input::{Key, KeySource};
use crate::keyboard::modifiers::Modifiers;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyFilter {
    Forward,
    Consume,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keystroke {
    pub seat: SeatId,
    pub code: u32,
    pub press: Press,
    pub time: Duration,
    pub source: KeySource,
    pub modifiers: Modifiers,
    pub sym: Keysym,
    pub latin: Option<Keysym>,
    pub shortcuts_inhibited: bool,
}

impl Keystroke {
    pub(crate) fn read(
        seat: SeatId,
        key: &Key,
        modifiers: Modifiers,
        state: &xkb::State,
        keymap: &xkb::Keymap,
        shortcuts_inhibited: bool,
    ) -> Self {
        let keycode = Keycode::new(key.code + 8);
        Self {
            seat,
            code: key.code,
            press: key.press,
            time: key.time,
            source: key.source,
            modifiers,
            sym: state.key_get_one_sym(keycode),
            latin: latin(state, keymap, keycode),
            shortcuts_inhibited,
        }
    }
}

fn latin(state: &xkb::State, keymap: &xkb::Keymap, keycode: Keycode) -> Option<Keysym> {
    let effective = state.key_get_layout(keycode);
    let base = *keymap
        .key_get_syms_by_level(keycode, effective, 0)
        .first()?;
    if base.key_char().is_none_or(|ch| ch.is_ascii()) {
        return Some(base);
    }
    let found = (0..keymap.num_layouts_for_key(keycode))
        .filter(|layout| *layout != effective)
        .filter_map(|layout| keymap.key_get_syms_by_level(keycode, layout, 0).first())
        .find(|sym| {
            sym.key_char()
                .is_some_and(|ch| ch.is_ascii() && !ch.is_ascii_control())
        });
    Some(found.copied().unwrap_or(base))
}
