use rikka_compositor_desktop::WindowId;
use rikka_input::{ActionMap, Chord};
use rikka_session::shortcuts::Shortcut;
use rikka_types::{Key, Modifiers};

use crate::settings::SessionSettings;

const VT_COUNT: u8 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyAction {
    Run(Shortcut),
    SwitchVt(i32),
    Switched(Option<WindowId>),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Bindings {
    map: ActionMap<KeyAction>,
}

impl Bindings {
    pub(crate) fn read(settings: Option<&SessionSettings>, switches_vt: bool) -> Self {
        let mut map = settings
            .map(SessionSettings::action_map)
            .unwrap_or_default();
        if switches_vt {
            for vt in 1..=VT_COUNT {
                map = map.bind(
                    Chord::new(Modifiers::CONTROL | Modifiers::ALT, Key::F(vt)),
                    KeyAction::SwitchVt(i32::from(vt)),
                );
            }
        }
        Self { map }
    }

    pub(crate) fn resolve(&self, modifiers: Modifiers, key: Key) -> Option<KeyAction> {
        self.map.resolve(modifiers, key).copied()
    }
}
