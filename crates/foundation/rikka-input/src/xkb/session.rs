use rikka_sync::{Mutex, MutexGuard};
use std::sync::Arc;

static SESSION_KEYBOARD: Mutex<Option<SessionKeyboard>> = Mutex::new(None);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionKeyboard {
    keymap: Option<Arc<str>>,
    repeat: Option<KeyRepeat>,
    generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyRepeat {
    pub rate: i32,
    pub delay: i32,
}

impl SessionKeyboard {
    pub fn publish_keymap(text: impl Into<Arc<str>>) {
        let text = text.into();
        update(|keyboard| {
            let changed = keyboard.keymap.as_ref() != Some(&text);
            keyboard.keymap = Some(text);
            changed
        });
    }

    pub fn publish_repeat(repeat: KeyRepeat) {
        update(|keyboard| keyboard.repeat.replace(repeat) != Some(repeat));
    }

    #[must_use]
    pub fn current() -> Option<SessionKeyboard> {
        lock_session_keyboard().clone()
    }

    #[must_use]
    pub fn keymap(&self) -> Option<&str> {
        self.keymap.as_deref()
    }

    #[must_use]
    pub fn repeat(&self) -> Option<KeyRepeat> {
        self.repeat
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

fn update(apply: impl FnOnce(&mut SessionKeyboard) -> bool) {
    let mut session_keyboard = lock_session_keyboard();
    let keyboard = session_keyboard.get_or_insert(SessionKeyboard {
        keymap: None,
        repeat: None,
        generation: 0,
    });
    if apply(keyboard) {
        keyboard.generation += 1;
    }
}

fn lock_session_keyboard() -> MutexGuard<'static, Option<SessionKeyboard>> {
    SESSION_KEYBOARD.lock()
}
