use rikka_text_document::{Erase, Travel};
use rikka_types::{Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyEdit {
    Travel { to: Travel, extend: bool },
    Erase(Erase),
    Newline,
    Tab { back: bool },
    Type(char),
}

impl KeyEdit {
    #[must_use]
    pub fn from_key(key: Key, modifiers: Modifiers, multiline: bool) -> Option<KeyEdit> {
        let command = modifiers.control && !modifiers.alt && !modifiers.logo;
        let travel = |to| {
            Some(KeyEdit::Travel {
                to,
                extend: modifiers.shift,
            })
        };
        match key {
            Key::ArrowLeft if command => travel(Travel::WordLeft),
            Key::ArrowLeft => travel(Travel::Left),
            Key::ArrowRight if command => travel(Travel::WordRight),
            Key::ArrowRight => travel(Travel::Right),
            Key::Home if multiline && !command => travel(Travel::LineStart),
            Key::Home => travel(Travel::Start),
            Key::End if multiline && !command => travel(Travel::LineEnd),
            Key::End => travel(Travel::End),
            Key::Backspace if command => Some(KeyEdit::Erase(Erase::WordBack)),
            Key::Backspace => Some(KeyEdit::Erase(Erase::Back)),
            Key::Delete if command => Some(KeyEdit::Erase(Erase::WordForward)),
            Key::Delete if !modifiers.shift => Some(KeyEdit::Erase(Erase::Forward)),
            Key::Enter if multiline => Some(KeyEdit::Newline),
            Key::Tab => Some(KeyEdit::Tab {
                back: modifiers.shift,
            }),
            Key::Space if !command => Some(KeyEdit::Type(' ')),
            Key::Char(typed) if !command && !modifiers.alt && !modifiers.logo => {
                Some(KeyEdit::Type(typed))
            }
            _ => None,
        }
    }
}
