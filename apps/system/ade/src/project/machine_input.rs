use rikka_input::Chord;
use rikka_types::{Modifiers, PointerButton};

#[derive(Clone, Debug)]
pub(crate) enum MachineInput {
    PointerMove(u32, u32),
    PointerPress {
        button: PointerButton,
        at: (u32, u32),
        modifiers: Modifiers,
    },
    PointerRelease {
        button: PointerButton,
        at: (u32, u32),
    },
    Scroll {
        at: (u32, u32),
        across: f64,
        down: f64,
    },
    Key(Chord),
    Text(String),
}
