use std::cell::OnceCell;
use std::rc::Rc;

use rikka_virtual_input::Keyboard;
use shell_kit::Typist;

use crate::layout;

#[derive(Clone, Debug)]
pub(crate) struct Board {
    typist: Typist,
    keyboard: Rc<OnceCell<Option<Keyboard>>>,
}

impl Board {
    #[must_use]
    pub(crate) fn new(typist: Typist) -> Board {
        Board {
            typist,
            keyboard: Rc::default(),
        }
    }

    pub(crate) fn keyboard(&self) -> Option<&Keyboard> {
        self.keyboard
            .get_or_init(|| {
                self.typist
                    .seat()?
                    .keyboard(layout::caps())
                    .inspect_err(|error| {
                        tracing::warn!(
                            "on-screen keyboard cannot connect to virtual keyboard: {error}"
                        );
                    })
                    .ok()
            })
            .as_ref()
    }
}
