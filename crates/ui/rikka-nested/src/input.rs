use std::ops::Range;

use rikka_types::{Gesture, KeyCode, Point, PointerButton, ScrollDelta};
use rikka_ui::{DragAction, DragOffer, ScrollSource};

#[derive(Clone, Debug)]
pub enum Input {
    PointerMoved {
        at: Point,
    },
    PointerLeft,
    Button {
        at: Point,
        button: PointerButton,
        pressed: bool,
    },
    Scroll {
        at: Point,
        delta: ScrollDelta,
        source: ScrollSource,
    },
    Gesture {
        at: Point,
        gesture: Gesture,
    },
    Travelled {
        delta: Point,
    },
    Key {
        code: KeyCode,
        pressed: bool,
    },
    Preedit {
        text: String,
        cursor: Option<Range<usize>>,
    },
    Commit(String),
    DeleteSurrounding {
        before: u32,
        after: u32,
    },
    Focus(bool),
    TouchDown {
        slot: u32,
        at: Point,
    },
    TouchMoved {
        slot: u32,
        at: Point,
    },
    TouchUp {
        slot: u32,
    },
    TouchCancelled,
    DragMoved {
        at: Point,
        offer: DragOffer,
    },
    DragLeft,
    DragDropped {
        at: Point,
        offer: DragOffer,
        action: DragAction,
    },
}
