use std::collections::BTreeMap;

use cut_project::{Appearance, ClipId, EffectSlot, Graphic};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overrides {
    appearances: BTreeMap<ClipId, Appearance>,
    effects: BTreeMap<ClipId, Vec<EffectSlot>>,
    drawing: Option<Graphic>,
}

impl Overrides {
    pub fn show(&mut self, clip: ClipId, appearance: Appearance) -> bool {
        if self.appearances.get(&clip) == Some(&appearance) {
            return false;
        }
        self.appearances.insert(clip, appearance);
        true
    }

    pub fn show_effects(&mut self, clip: ClipId, effects: Vec<EffectSlot>) -> bool {
        if self.effects.get(&clip) == Some(&effects) {
            return false;
        }
        self.effects.insert(clip, effects);
        true
    }

    pub fn draw(&mut self, drawing: Option<Graphic>) {
        self.drawing = drawing;
    }

    pub fn clear(&mut self) {
        self.appearances.clear();
        self.effects.clear();
        self.drawing = None;
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.appearances.is_empty() && self.effects.is_empty() && self.drawing.is_none()
    }

    #[must_use]
    pub fn appearance(&self, clip: ClipId) -> Option<&Appearance> {
        self.appearances.get(&clip)
    }

    #[must_use]
    pub fn effects(&self, clip: ClipId) -> Option<&[EffectSlot]> {
        self.effects.get(&clip).map(Vec::as_slice)
    }

    #[must_use]
    pub fn drawing(&self) -> Option<&Graphic> {
        self.drawing.as_ref()
    }
}
