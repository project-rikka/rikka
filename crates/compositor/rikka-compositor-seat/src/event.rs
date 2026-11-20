use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_surface::SurfaceId;

use crate::focus::KeyboardFocus;
use crate::ids::{SeatId, ToolId};
use crate::ime::TextInputState;
use crate::keyboard::{KeyboardGrab, Modifiers};
use crate::pointer::{ConstraintState, CursorImage};
use crate::virtual_pointer::VirtualPointerInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caret {
    pub surface: SurfaceId,
    pub rect: Rect<i32, Logical>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SeatEvent {
    KeyboardFocus {
        seat: SeatId,
        focus: Option<KeyboardFocus>,
    },
    KeyboardGrab {
        seat: SeatId,
        grab: KeyboardGrab,
        active: bool,
    },
    TouchGrab {
        seat: SeatId,
        grabbed: bool,
    },
    PointerGrab {
        seat: SeatId,
        grabbed: bool,
    },
    Cursor {
        seat: SeatId,
        image: CursorImage,
    },
    ToolCursor {
        seat: SeatId,
        tool: ToolId,
        image: CursorImage,
    },
    Constraint {
        seat: SeatId,
        surface: SurfaceId,
        state: ConstraintState,
    },
    CursorHint {
        seat: SeatId,
        location: Point<f64, Logical>,
    },
    Warp {
        seat: SeatId,
        location: Point<f64, Logical>,
    },
    Caret {
        seat: SeatId,
        caret: Option<Caret>,
    },
    InputPanel {
        seat: SeatId,
        surface: SurfaceId,
        shown: bool,
    },
    InputPopup {
        seat: SeatId,
        popup: SurfaceId,
        parent: Option<SurfaceId>,
        cursor: Option<Rect<i32, Logical>>,
    },
    InputPopupGone {
        seat: SeatId,
        popup: SurfaceId,
    },
    VirtualPointer {
        seat: SeatId,
        output: Option<WlOutput>,
        input: VirtualPointerInput,
    },
    TextInput {
        seat: SeatId,
        state: Option<TextInputState>,
    },
    Modifiers {
        seat: SeatId,
        held: Modifiers,
    },
    IdleInhibitors,
}
