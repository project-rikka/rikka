use rikka_types::Key;
use rikka_ui::MaterialSymbol;
use rikka_virtual_input::Cap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Press {
    Types(Cap),
    Holds(Modifier),
    Locks,
    Hides,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Modifier {
    Shift,
    Control,
    Alt,
    Logo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Face {
    Word(&'static str),
    Symbol(MaterialSymbol),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Slot {
    pub(crate) press: Press,
    pub(crate) face: Option<Face>,
    pub(super) units: f32,
}

const fn slot(press: Press, face: Face, units: f32) -> Slot {
    Slot {
        press,
        face: Some(face),
        units,
    }
}

const fn character(plain: char, shifted: char) -> Slot {
    Slot {
        press: Press::Types(Cap::Levels(plain, shifted)),
        face: None,
        units: 1.0,
    }
}

const fn named(key: Key, face: Face, units: f32) -> Slot {
    slot(Press::Types(Cap::Named(key)), face, units)
}

const fn modifier_key(modifier: Modifier, word: &'static str, units: f32) -> Slot {
    slot(Press::Holds(modifier), Face::Word(word), units)
}

pub(crate) const UNITS: f32 = 15.0;

pub(crate) const ROWS: &[&[Slot]] = &[
    &[
        named(Key::Escape, Face::Word("Esc"), 1.0),
        named(Key::F(1), Face::Word("F1"), 1.0),
        named(Key::F(2), Face::Word("F2"), 1.0),
        named(Key::F(3), Face::Word("F3"), 1.0),
        named(Key::F(4), Face::Word("F4"), 1.0),
        named(Key::F(5), Face::Word("F5"), 1.0),
        named(Key::F(6), Face::Word("F6"), 1.0),
        named(Key::F(7), Face::Word("F7"), 1.0),
        named(Key::F(8), Face::Word("F8"), 1.0),
        named(Key::F(9), Face::Word("F9"), 1.0),
        named(Key::F(10), Face::Word("F10"), 1.0),
        named(Key::F(11), Face::Word("F11"), 1.0),
        named(Key::F(12), Face::Word("F12"), 1.0),
        named(Key::PrintScreen, Face::Word("PrtSc"), 1.0),
        named(Key::Delete, Face::Word("Del"), 1.0),
    ],
    &[
        character('`', '~'),
        character('1', '!'),
        character('2', '@'),
        character('3', '#'),
        character('4', '$'),
        character('5', '%'),
        character('6', '^'),
        character('7', '&'),
        character('8', '*'),
        character('9', '('),
        character('0', ')'),
        character('-', '_'),
        character('=', '+'),
        named(Key::Backspace, Face::Symbol(MaterialSymbol::BACKSPACE), 2.0),
    ],
    &[
        named(Key::Tab, Face::Symbol(MaterialSymbol::KEYBOARD_TAB), 1.5),
        character('q', 'Q'),
        character('w', 'W'),
        character('e', 'E'),
        character('r', 'R'),
        character('t', 'T'),
        character('y', 'Y'),
        character('u', 'U'),
        character('i', 'I'),
        character('o', 'O'),
        character('p', 'P'),
        character('[', '{'),
        character(']', '}'),
        Slot {
            press: Press::Types(Cap::Levels('\\', '|')),
            face: None,
            units: 1.5,
        },
    ],
    &[
        slot(
            Press::Locks,
            Face::Symbol(MaterialSymbol::KEYBOARD_CAPSLOCK),
            1.75,
        ),
        character('a', 'A'),
        character('s', 'S'),
        character('d', 'D'),
        character('f', 'F'),
        character('g', 'G'),
        character('h', 'H'),
        character('j', 'J'),
        character('k', 'K'),
        character('l', 'L'),
        character(';', ':'),
        character('\'', '"'),
        named(
            Key::Enter,
            Face::Symbol(MaterialSymbol::KEYBOARD_RETURN),
            2.25,
        ),
    ],
    &[
        modifier_key(Modifier::Shift, "Shift", 2.25),
        character('z', 'Z'),
        character('x', 'X'),
        character('c', 'C'),
        character('v', 'V'),
        character('b', 'B'),
        character('n', 'N'),
        character('m', 'M'),
        character(',', '<'),
        character('.', '>'),
        character('/', '?'),
        modifier_key(Modifier::Shift, "Shift", 1.75),
        named(
            Key::ArrowUp,
            Face::Symbol(MaterialSymbol::ARROW_UPWARD),
            1.0,
        ),
    ],
    &[
        modifier_key(Modifier::Control, "Ctrl", 1.25),
        modifier_key(Modifier::Logo, "Super", 1.25),
        modifier_key(Modifier::Alt, "Alt", 1.25),
        named(Key::Space, Face::Word("Space"), 5.0),
        modifier_key(Modifier::Alt, "Alt", 1.25),
        named(Key::Menu, Face::Symbol(MaterialSymbol::MENU), 1.0),
        named(
            Key::ArrowLeft,
            Face::Symbol(MaterialSymbol::ARROW_BACK),
            1.0,
        ),
        named(
            Key::ArrowDown,
            Face::Symbol(MaterialSymbol::ARROW_DOWNWARD),
            1.0,
        ),
        named(
            Key::ArrowRight,
            Face::Symbol(MaterialSymbol::ARROW_FORWARD),
            1.0,
        ),
        slot(
            Press::Hides,
            Face::Symbol(MaterialSymbol::KEYBOARD_HIDE),
            1.0,
        ),
    ],
];

pub(crate) fn caps() -> impl Iterator<Item = Cap> {
    ROWS.iter().flat_map(|row| row.iter()).filter_map(|slot| {
        let Press::Types(cap) = slot.press else {
            return None;
        };
        Some(cap)
    })
}

pub(super) fn slot_at(row: usize, index: usize) -> Option<Slot> {
    ROWS.get(row)?.get(index).copied()
}
