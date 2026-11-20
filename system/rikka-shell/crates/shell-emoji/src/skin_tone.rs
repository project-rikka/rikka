use std::cell::Cell;
use std::rc::Rc;

use rikka_emoji::Tone;
use rikka_seam::SeamClient;
use rikka_settings::seam::protocol;
use rikka_settings::session::emoji::{DOMAIN, SKIN_TONE};
use rikka_types::Choice;

#[derive(Clone, Debug, Default)]
pub(super) struct SkinTone(Rc<Cell<Tone>>);

impl SkinTone {
    #[must_use]
    pub(crate) fn tone(&self) -> Tone {
        self.0.get()
    }

    pub(crate) fn set(&self, tone: Tone) {
        self.0.set(tone);
    }

    pub(crate) fn ask(&self, seam: &SeamClient, tone: Tone) {
        self.set(tone);
        seam.notify(protocol::set(DOMAIN, SKIN_TONE, tone.slug()));
    }
}
