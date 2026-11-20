use cut_project::{Clip, Edit, EffectSlot};
use rikka_app::prelude::*;
use rikka_effects::{Effect, Kind};

use super::Cut;

impl Cut {
    pub(crate) fn selected_visual(&self) -> Option<&Clip> {
        self.selected_clip()
            .filter(|clip| self.project().is_visual(clip))
    }

    fn restack(&mut self, change: impl FnOnce(&mut Vec<EffectSlot>) -> bool) {
        self.session.popup = None;
        self.live.settle();
        let Some(clip) = self.selected_visual() else {
            return;
        };
        let id = clip.id();
        let mut effects = clip.effects().to_vec();
        if change(&mut effects) {
            self.edit(Edit::Effects { clip: id, effects });
        }
    }

    pub(crate) fn add_effect(&mut self, kind: Kind) {
        self.restack(|effects| {
            effects.push(EffectSlot::new(kind.default_effect()));
            true
        });
    }

    pub(crate) fn remove_effect(&mut self, slot: usize) {
        self.restack(|effects| {
            let exists = slot < effects.len();
            if exists {
                effects.remove(slot);
            }
            exists
        });
    }

    pub(crate) fn move_effect(&mut self, slot: usize, later: bool) {
        self.restack(|effects| {
            let target = match later {
                true => slot.checked_add(1),
                false => slot.checked_sub(1),
            };
            match target.filter(|target| *target < effects.len() && slot < effects.len()) {
                Some(target) => {
                    effects.swap(slot, target);
                    true
                }
                None => false,
            }
        });
    }

    pub(crate) fn enable_effect(&mut self, slot: usize, on: bool) {
        self.restack(|effects| match effects.get_mut(slot) {
            Some(effect_slot) if effect_slot.enabled != on => {
                effect_slot.enabled = on;
                true
            }
            _ => false,
        });
    }

    pub(crate) fn preview_effect(&mut self, slot: usize, effect: Effect) {
        let frame = self.transport.frame();
        if let Some(clip) = self.selected_visual()
            && let Some(effects) = clip.retuned(slot, effect, frame)
        {
            self.live.show_effects(clip.id(), effects);
        }
        skip_view();
    }

    pub(crate) fn retune_effect(&mut self, slot: usize, effect: Effect) {
        let frame = self.transport.frame();
        self.live.settle();
        let Some(clip) = self.selected_visual() else {
            skip_view();
            return;
        };
        let id = clip.id();
        match clip.retuned(slot, effect, frame) {
            Some(effects) if effects != clip.effects() => {
                self.edit(Edit::Effects { clip: id, effects });
            }
            _ => skip_view(),
        }
    }
}
