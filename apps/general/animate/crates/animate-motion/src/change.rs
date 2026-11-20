use crate::take::Take;

impl Take {
    #[must_use]
    pub fn changed_from(&self, before: &Take) -> Option<u32> {
        let human = match (&self.human, &before.human) {
            (Some(now), Some(then)) => now.dirty_since(then).map(|dirty| dirty.start),
            (None, None) => None,
            _ => Some(0),
        };
        [
            human,
            self.bones.changed_from(&before.bones),
            self.switches.changed_from(&before.switches),
            self.morphs.changed_from(&before.morphs),
            self.holds.changed_since(&before.holds),
        ]
        .into_iter()
        .flatten()
        .min()
    }
}
