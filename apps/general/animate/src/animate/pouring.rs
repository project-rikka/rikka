use animate_fluid::{Emitter, EmitterState};

use crate::animate::Animate;

impl Animate {
    pub(crate) fn pouring_now(&self, emitter: &Emitter) -> EmitterState {
        let frame = self.transport.frame();
        let mut state = emitter.sample(frame as f32);
        if let Some(held) = self.stage.lock().drafts().pour.at(emitter.id, frame) {
            held.apply(|setting, value| state.set(setting, value));
        }
        state
    }
}
