use std::ffi::OsString;
use std::os::fd::OwnedFd;

use rikka_types::{Key, KeyCode, KeyLocation, Modifiers};
use xkbcommon::xkb::{self, Keycode, compose};
use xkeysym::Keysym;

use crate::error::{Error, Result};
use crate::evdev::XKB_OFFSET;

pub struct Keyboard {
    context: xkb::Context,
    state: Option<xkb::State>,
    compose: Option<compose::State>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keystroke {
    code: u32,
    keysym: Keysym,
    text: Option<String>,
}

impl Keystroke {
    #[must_use]
    pub fn key(&self) -> Key {
        super::key_with_text(self.keysym, self.text.as_deref())
    }

    #[must_use]
    pub fn code(&self) -> KeyCode {
        KeyCode::from_evdev(self.code)
    }

    #[must_use]
    pub fn location(&self) -> KeyLocation {
        super::location(self.keysym)
    }

    #[must_use]
    pub fn is_key(&self, code: u32) -> bool {
        self.code == code
    }
}

impl Keyboard {
    #[must_use]
    pub fn new() -> Keyboard {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let compose =
            compose::Table::new_from_locale(&context, &locale(), compose::COMPILE_NO_FLAGS)
                .ok()
                .map(|table| compose::State::new(&table, compose::STATE_NO_FLAGS));
        Keyboard {
            context,
            state: None,
            compose,
        }
    }

    pub unsafe fn load(&mut self, keymap: OwnedFd, size: usize) -> Result<()> {
        let compiled = unsafe {
            xkb::Keymap::new_from_fd(
                &self.context,
                keymap,
                size,
                xkb::KEYMAP_FORMAT_TEXT_V1,
                xkb::COMPILE_NO_FLAGS,
            )
        }
        .map_err(|error| Error::KeymapRead(error.to_string()))?
        .ok_or(Error::InvalidKeymap)?;
        self.state = Some(xkb::State::new(&compiled));
        Ok(())
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.state.is_some()
    }

    #[must_use]
    pub fn keymap_text(&self) -> Option<String> {
        let state = self.state.as_ref()?;
        Some(state.get_keymap().get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1))
    }

    #[must_use]
    pub fn is_caps_locked(&self) -> bool {
        self.state.as_ref().is_some_and(|state| {
            state.mod_name_is_active(xkb::MOD_NAME_CAPS, xkb::STATE_MODS_LOCKED)
        })
    }

    #[must_use]
    pub fn layout(&self) -> Option<String> {
        let state = self.state.as_ref()?;
        let keymap = state.get_keymap();
        let name = keymap.layout_get_name(state.serialize_layout(xkb::STATE_LAYOUT_EFFECTIVE));
        (!name.is_empty()).then(|| name.to_owned())
    }

    #[must_use]
    pub fn layout_count(&self) -> u32 {
        self.state
            .as_ref()
            .map_or(0, |state| state.get_keymap().num_layouts())
    }

    pub fn press(&mut self, code: u32) -> Option<Keystroke> {
        let keysym = self.state.as_ref()?.key_get_one_sym(keycode(code));
        let text = self.text(code, keysym);
        Some(Keystroke { code, keysym, text })
    }

    #[must_use]
    pub fn keystroke(&self, code: u32) -> Option<Keystroke> {
        let keysym = self.state.as_ref()?.key_get_one_sym(keycode(code));
        Some(Keystroke {
            code,
            keysym,
            text: None,
        })
    }

    #[must_use]
    pub fn key_repeats(&self, code: u32) -> bool {
        self.state
            .as_ref()
            .is_some_and(|state| state.get_keymap().key_repeats(keycode(code)))
    }

    pub fn refresh_text(&self, keystroke: &mut Keystroke) {
        if let Some(state) = self.state.as_ref() {
            keystroke.text = Some(state.key_get_utf8(keycode(keystroke.code)));
        }
    }

    pub fn set_modifiers(
        &mut self,
        depressed: u32,
        latched: u32,
        locked: u32,
        group: u32,
    ) -> Option<Modifiers> {
        let state = self.state.as_mut()?;
        state.update_mask(depressed, latched, locked, 0, 0, group);
        let active = |name: &str| state.mod_name_is_active(name, xkb::STATE_MODS_EFFECTIVE);
        Some(Modifiers {
            shift: active(xkb::MOD_NAME_SHIFT),
            control: active(xkb::MOD_NAME_CTRL),
            alt: active(xkb::MOD_NAME_ALT),
            logo: active(xkb::MOD_NAME_LOGO),
        })
    }

    fn text(&mut self, code: u32, keysym: Keysym) -> Option<String> {
        let state = self.state.as_ref()?;
        let Some(compose) = self.compose.as_mut() else {
            return Some(state.key_get_utf8(keycode(code)));
        };
        match compose.feed(keysym) {
            compose::FeedResult::Ignored => None,
            compose::FeedResult::Accepted => match compose.status() {
                compose::Status::Composed => compose.utf8(),
                compose::Status::Nothing => Some(state.key_get_utf8(keycode(code))),
                compose::Status::Composing | compose::Status::Cancelled => None,
            },
        }
    }
}

impl Default for Keyboard {
    fn default() -> Keyboard {
        Keyboard::new()
    }
}

impl std::fmt::Debug for Keyboard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Keyboard")
            .field("ready", &self.is_ready())
            .field("composes", &self.compose.is_some())
            .finish_non_exhaustive()
    }
}

fn keycode(code: u32) -> Keycode {
    Keycode::new(code + XKB_OFFSET)
}

fn locale() -> OsString {
    rikka_types::LocaleCategory::Ctype
        .posix_name()
        .map_or_else(|| "C".into(), OsString::from)
}
