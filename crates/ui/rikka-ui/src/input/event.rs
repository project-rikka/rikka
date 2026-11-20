use rikka_input::{Command, DeviceEvent, ScrollSource};
use rikka_types::{
    DragAction, Gesture, Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, ScrollDelta,
};

use crate::input::{DragEnd, DragOffer, PadInput};

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
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
    PointerCancelled,
    PointerLeft,
    PointerEntered,
    PointerExited,
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
    Preedit {
        text: String,
        cursor: Option<std::ops::Range<usize>>,
    },
    Commit(String),
    KeyReleased {
        key: Key,
        code: KeyCode,
        location: KeyLocation,
        modifiers: Modifiers,
    },
    Pad(PadInput),
    Command(Command),
    FocusGained,
    FocusLost,
    DragMoved {
        position: Point,
        offer: DragOffer,
    },
    DragLeft,
    DragDropped {
        position: Point,
        offer: DragOffer,
        action: DragAction,
    },
    DragTaken,
    DragWithin(bool),
    DragReclaimed,
    DragEnded(DragEnd),
}

impl From<DeviceEvent> for Event {
    fn from(event: DeviceEvent) -> Event {
        match event {
            DeviceEvent::PointerMoved(position) => Event::PointerMoved(position),
            DeviceEvent::PointerTravelled(by) => Event::PointerTravelled(by),
            DeviceEvent::PointerPressed {
                position,
                button,
                modifiers,
            } => Event::PointerPressed {
                position,
                button,
                modifiers,
            },
            DeviceEvent::PointerReleased {
                position,
                button,
                modifiers,
            } => Event::PointerReleased {
                position,
                button,
                modifiers,
            },
            DeviceEvent::PointerLeft => Event::PointerLeft,
            DeviceEvent::Scroll {
                position,
                delta,
                modifiers,
                source,
            } => Event::Scroll {
                position,
                delta,
                modifiers,
                source,
            },
            DeviceEvent::Gesture { position, gesture } => Event::Gesture { position, gesture },
            DeviceEvent::KeyPressed {
                key,
                code,
                location,
                modifiers,
            } => Event::KeyPressed {
                key,
                code,
                location,
                modifiers,
            },
            DeviceEvent::KeyReleased {
                key,
                code,
                location,
                modifiers,
            } => Event::KeyReleased {
                key,
                code,
                location,
                modifiers,
            },
            DeviceEvent::Preedit { text, cursor } => Event::Preedit { text, cursor },
            DeviceEvent::Commit(text) => Event::Commit(text),
        }
    }
}
