use std::ops::Range;

use rikka_types::{
    Gesture, Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, ScrollDelta,
};

#[derive(Clone, Debug, PartialEq)]
pub enum DeviceEvent {
    PointerMoved(Point),
    PointerTravelled(Point),
    PointerPressed {
        position: Point,
        button: PointerButton,
        modifiers: Modifiers,
    },
    PointerReleased {
        position: Point,
        button: PointerButton,
        modifiers: Modifiers,
    },
    PointerLeft,
    Scroll {
        position: Point,
        delta: ScrollDelta,
        modifiers: Modifiers,
        source: ScrollSource,
    },
    Gesture {
        position: Point,
        gesture: Gesture,
    },
    KeyPressed {
        key: Key,
        code: KeyCode,
        location: KeyLocation,
        modifiers: Modifiers,
    },
    KeyReleased {
        key: Key,
        code: KeyCode,
        location: KeyLocation,
        modifiers: Modifiers,
    },
    Preedit {
        text: String,
        cursor: Option<Range<usize>>,
    },
    Commit(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollSource {
    Wheel,
    Continuous(ScrollPhase),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollPhase {
    Began,
    Moved,
    Ended,
}

impl ScrollSource {
    #[must_use]
    pub fn phase(self) -> Option<ScrollPhase> {
        match self {
            ScrollSource::Wheel => None,
            ScrollSource::Continuous(phase) => Some(phase),
        }
    }
}
