use std::cell::OnceCell;
use std::rc::Rc;

use rikka_virtual_input::{Seat, TextKeyboard};

#[derive(Clone, Default)]
pub struct Typist(Rc<Devices>);

#[derive(Default)]
struct Devices {
    seat: OnceCell<Option<Seat>>,
    text: OnceCell<Option<TextKeyboard>>,
}

impl Typist {
    #[must_use]
    pub fn seat(&self) -> Option<&Seat> {
        self.0
            .seat
            .get_or_init(|| {
                Seat::connect()
                    .inspect_err(|error| tracing::warn!("no virtual input seat: {error}"))
                    .ok()
            })
            .as_ref()
    }

    #[must_use]
    pub fn text_keyboard(&self) -> Option<&TextKeyboard> {
        self.0
            .text
            .get_or_init(|| {
                self.seat()?
                    .text_keyboard()
                    .inspect_err(|error| {
                        tracing::warn!("no virtual keyboard to type with: {error}")
                    })
                    .ok()
            })
            .as_ref()
    }
}

impl std::fmt::Debug for Typist {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Typist")
            .field("connected", &self.0.seat.get().is_some_and(Option::is_some))
            .finish()
    }
}
