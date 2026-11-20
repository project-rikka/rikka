use crate::emitter::Emitter;

impl Emitter {
    #[must_use]
    pub fn changed_from(&self, before: &Emitter) -> Option<u32> {
        let reshaped = self.shape != before.shape
            || self.aim != before.aim
            || self.launch != before.launch
            || self.target != before.target
            || self.levels != before.levels
            || self.liquid.liquid() != before.liquid.liquid();
        if reshaped {
            return Some(0);
        }
        self.tracks()
            .changed_from(before.tracks())
            .into_iter()
            .chain(self.repoured_from(before))
            .min()
    }

    fn repoured_from(&self, before: &Emitter) -> Option<u32> {
        let (one, two) = (before.pour, self.pour);
        if one.start() != two.start() {
            return Some(one.start().min(two.start()));
        }
        let ends = self.shape.flows() && one.end() != two.end();
        ends.then(|| one.end().min(two.end()))
    }
}
