use std::cell::RefCell;
use std::rc::Rc;

use rikka_types::{Point, ResizeEdge, WindowIdentifier};

use crate::text::Label;

thread_local! {
    static IDENTIFIER: RefCell<Option<Rc<WindowIdentifier>>> = const { RefCell::new(None) };
}

#[must_use]
pub fn window_identifier() -> Option<WindowIdentifier> {
    IDENTIFIER.with_borrow(|identifier| identifier.as_deref().cloned())
}

pub(crate) struct Identified(Option<Rc<WindowIdentifier>>);

pub(crate) fn identified(identifier: Option<Rc<WindowIdentifier>>) -> Identified {
    Identified(IDENTIFIER.with_borrow_mut(|current| std::mem::replace(current, identifier)))
}

impl Drop for Identified {
    fn drop(&mut self) {
        let outer = self.0.take();
        IDENTIFIER.with_borrow_mut(|current| *current = outer);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum WindowRequest {
    Move,
    Resize(ResizeEdge),
    Minimize,
    ToggleMaximize,
    AskToClose,
    Close,
    ShowMenu(Point),
    Fullscreen(bool),
    ToggleFullscreen,
    Activate(String),
    Raise(WindowRef),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WindowPart {
    Caption,
    Minimize,
    Maximize,
    Close,
    Resize(ResizeEdge),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WindowRef(u64);

impl WindowRef {
    #[must_use]
    pub fn new(raw: u64) -> WindowRef {
        WindowRef(raw)
    }

    #[must_use]
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowChrome {
    pub title: Label,
    pub focused: bool,
    pub maximized: bool,
    pub fullscreen: bool,
    pub resizable: bool,
}

impl WindowChrome {
    pub fn new(title: impl Into<Label>) -> Self {
        Self {
            title: title.into(),
            focused: true,
            maximized: false,
            fullscreen: false,
            resizable: true,
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn maximized(mut self, maximized: bool) -> Self {
        self.maximized = maximized;
        self
    }

    pub fn fullscreen(mut self, fullscreen: bool) -> Self {
        self.fullscreen = fullscreen;
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }
}

impl Default for WindowChrome {
    fn default() -> Self {
        Self::new("")
    }
}
