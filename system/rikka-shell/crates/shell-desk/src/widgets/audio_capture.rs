use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rikka_sound::Spectrum;
use rikka_ui::Observers;
use shell_feeds::AudioSpectrum;

#[derive(Default)]
pub(super) struct AudioCapture {
    holders: Cell<usize>,
    listener: RefCell<Option<Rc<Spectrum>>>,
    observers: Observers,
}

impl AudioCapture {
    #[must_use]
    pub(crate) fn spectrum(&self) -> Option<Spectrum> {
        self.observers.read();
        self.listener.borrow().as_deref().cloned()
    }

    pub(crate) fn hold(&self, audio_spectrum: &AudioSpectrum) {
        self.holders.set(self.holders.get() + 1);
        if self.listener.borrow().is_none() {
            *self.listener.borrow_mut() = audio_spectrum.listen();
            self.observers.changed();
        }
    }

    pub(crate) fn release(&self) {
        let holders = self.holders.get().saturating_sub(1);
        self.holders.set(holders);
        if holders == 0 && self.listener.take().is_some() {
            self.observers.changed();
        }
    }
}
