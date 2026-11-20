use animate_fluid::Pour;
use animate_inspector::PourTiming;

use crate::animate::Animate;

impl Animate {
    pub(crate) fn retime_pour(&mut self, pour: Pour) {
        self.update_emitter(|emitter| emitter.pour = pour);
    }
}

impl PourTiming for Animate {
    fn end_pour_here(&mut self) {
        let end = self.transport.frame();
        self.update_emitter(|emitter| emitter.pour = emitter.pour.ending_at(end));
    }
    fn pour_for(&mut self, seconds: f32) {
        self.update_emitter(|emitter| emitter.pour = emitter.pour.lasting_seconds(seconds));
    }
    fn pour_from(&mut self, start: u32) {
        self.update_emitter(|emitter| emitter.pour = emitter.pour.moved_to(start));
    }
    fn start_pour_here(&mut self) {
        self.pour_from(self.transport.frame());
    }
}
