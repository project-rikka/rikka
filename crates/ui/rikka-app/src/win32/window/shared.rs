use std::cell::{Cell, RefCell};

use rikka_types::{Rect, Size};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GWLP_USERDATA, GetWindowLongPtrW, SetWindowLongPtrW,
};

use crate::tasks::WindowId;
use crate::win32::input::Input;
use crate::win32::input::cursor::Pointer;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Frame {
    pub(crate) custom: bool,
    pub(crate) resizable: bool,
    pub(crate) fullscreen: bool,
    pub(crate) min_size: Option<Size>,
}

#[derive(Debug)]
pub(crate) struct Shared {
    pub(crate) id: WindowId,
    pub(crate) frame: Cell<Frame>,
    pub(crate) pointer: Cell<Pointer>,
    pub(crate) caret: Cell<Option<Rect>>,
    pub(crate) sizing: Cell<bool>,
    pub(crate) input: RefCell<Input>,
}

impl Shared {
    pub(crate) fn new(id: WindowId, frame: Frame, pointer: Pointer) -> Shared {
        Shared {
            id,
            frame: Cell::new(frame),
            pointer: Cell::new(pointer),
            caret: Cell::new(None),
            sizing: Cell::new(false),
            input: RefCell::new(Input::default()),
        }
    }
}

pub(crate) fn attach(window: HWND, shared: *const Shared) {
    unsafe { SetWindowLongPtrW(window, GWLP_USERDATA, shared as isize) };
}

pub(crate) fn detach(window: HWND) -> *const Shared {
    unsafe { SetWindowLongPtrW(window, GWLP_USERDATA, 0) as *const Shared }
}

pub(crate) fn attached(window: HWND) -> *const Shared {
    unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) as *const Shared }
}
