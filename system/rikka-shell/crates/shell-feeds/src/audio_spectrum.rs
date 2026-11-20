use std::cell::RefCell;
use std::rc::{Rc, Weak};

use rikka_sound::Spectrum;

const BANDS: usize = 24;

#[derive(Clone, Debug, Default)]
pub struct AudioSpectrum(Rc<RefCell<Weak<Spectrum>>>);

impl AudioSpectrum {
    #[must_use]
    pub fn listen(&self) -> Option<Rc<Spectrum>> {
        if let Some(shared) = self.0.borrow().upgrade() {
            return Some(shared);
        }
        let spectrum = Rc::new(
            Spectrum::listen(BANDS)
                .inspect_err(|error| tracing::warn!("cannot capture music audio: {error}"))
                .ok()?,
        );
        *self.0.borrow_mut() = Rc::downgrade(&spectrum);
        Some(spectrum)
    }
}
