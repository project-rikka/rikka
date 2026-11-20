use std::cell::RefCell;

use rikka_types::Stylus;
use rikka_ui::Event;

use crate::tasks::WindowId;

#[derive(Debug)]
pub(crate) enum Heard {
    Input {
        event: Event,
        stylus: Option<Stylus>,
    },
    Focus(bool),
    Resized,
    Rescaled,
    CloseRequested,
    Appearance,
    Displays,
}

thread_local! {
    static HEARD: RefCell<Vec<(WindowId, Heard)>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn hear(window: WindowId, heard: Heard) {
    HEARD.with_borrow_mut(|queue| queue.push((window, heard)));
}

pub(crate) fn input(window: WindowId, event: Event) {
    hear(
        window,
        Heard::Input {
            event,
            stylus: None,
        },
    );
}

pub(crate) fn take(into: &mut Vec<(WindowId, Heard)>) {
    HEARD.with_borrow_mut(|queue| into.append(queue));
}

pub(crate) fn is_empty() -> bool {
    HEARD.with_borrow(Vec::is_empty)
}
