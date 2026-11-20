use animate_inspector::ClothEditing;
use std::sync::Arc;

use animate_body::{Faces, MorphWeights};
use animate_cast::{FigureId, Subject};
use animate_edit::{Change, Key, KeyId, KeyKind};
use animate_inspector::MorphEditing;
use animate_motion::{MorphKey, Take};
use animate_sheet::Lane;
use rikka_app::prelude::TextDocument;
use rustc_hash::FxHashSet;

use crate::animate::Animate;

const NOTICED: f32 = 1e-4;

impl Animate {
    fn chosen_morphing(&self) -> Option<(FigureId, Arc<Faces>, Arc<Take>)> {
        let Some(Subject::Figure(id)) = self.subject else {
            return None;
        };
        let figure = self.cast.figure(id)?;
        Some((id, Arc::clone(&figure.faces), Arc::clone(&figure.take)))
    }

    fn pending_morphs(&self, id: FigureId) -> MorphWeights {
        self.stage
            .lock()
            .pending_at(id, self.transport.frame())
            .map(|pending| pending.morphs.clone())
            .unwrap_or_default()
    }

    pub(crate) fn zero_morphs(&mut self, chosen: Option<&str>) {
        self.express(|_, weights| {
            let morphs: Vec<String> = match chosen {
                Some(morph) => vec![morph.to_owned()],
                None => weights.iter().map(|(morph, _)| morph.to_owned()).collect(),
            };
            for morph in morphs {
                weights.set(&morph, 0.0);
            }
        });
    }

    pub(super) fn morph_keys(&self, id: FigureId, frame: u32, with_chosen: bool) -> Vec<Key> {
        let take = self.take_of(id);
        let pending = self.pending_morphs(id);
        let chosen = self
            .chosen_morph()
            .filter(|morph| with_chosen && !pending.holds(morph))
            .map(|morph| {
                let weight = take.morph_weight(&morph, frame as f32);
                self.morph_key(&take, &morph, frame, weight)
            });
        pending
            .iter()
            .map(|(morph, weight)| self.morph_key(&take, morph, frame, weight))
            .chain(chosen)
            .collect()
    }

    pub(super) fn morph_key(&self, take: &Take, morph: &str, frame: u32, weight: f32) -> Key {
        Key::Morph(MorphKey {
            morph: morph.to_owned(),
            frame,
            weight,
            curve: take
                .morphs
                .key(morph, frame)
                .map_or(self.curves.value(KeyKind::Morph), |standing| standing.curve),
        })
    }
}

impl MorphEditing for Animate {
    fn apply_typed_morphs(&mut self) {
        let Some(morphs) = self.morphing.typing.take() else {
            return;
        };
        let typed = self.morphing.entry.text().trim().replacen(',', ".", 1);
        if let Some(weight) = typed
            .parse::<f32>()
            .ok()
            .filter(|weight| weight.is_finite())
        {
            self.set_morphs(&morphs, weight.clamp(0.0, 1.0));
        }
    }
    fn choose_morph(&mut self, morph: &str) {
        self.timeline.choose_lane(Lane::Morph(morph.to_owned()));
    }
    fn chosen_faces(&self) -> Option<&Arc<Faces>> {
        self.chosen_figure().map(|figure| &figure.faces)
    }
    fn chosen_morph(&self) -> Option<String> {
        match self.timeline.chosen_lane() {
            Some(Lane::Morph(morph)) => Some(morph),
            _ => None,
        }
    }
    fn current_morph_weights(&self) -> Option<MorphWeights> {
        self.morphs_now().map(|(weights, _)| weights)
    }
    fn drop_typed_morphs(&mut self) {
        self.morphing.typing = None;
    }
    fn express(&mut self, change: &mut dyn FnMut(&Faces, &mut MorphWeights)) {
        Animate::express(self, change);
    }
    fn morph_entry(&self) -> &TextDocument {
        &self.morphing.entry
    }
    fn morph_query(&self) -> &TextDocument {
        &self.morphing.query
    }
    fn morphs_now(&self) -> Option<(MorphWeights, MorphWeights)> {
        let (id, _, take) = self.chosen_morphing()?;
        let pending = self.pending_morphs(id);
        let mut weights = take.morph_weights(self.transport.frame() as f32);
        weights.overlay(&pending);
        Some((weights, pending))
    }
    fn morphs_open(&self) -> bool {
        self.preferences.morphs_open()
    }
    fn revert_morphs(&mut self) {
        let Some(Subject::Figure(id)) = self.subject else {
            return;
        };
        let frame = self.transport.frame();
        self.stage
            .lock()
            .hold_morphs(id, frame, MorphWeights::default());
    }
    fn set_morphs(&mut self, morphs: &[String], weight: f32) {
        self.express(|_, weights| {
            for morph in morphs {
                weights.set(morph, weight);
            }
        });
    }
    fn show_morphs(&mut self, open: bool) {
        self.preferences.keep_morphs_open(open);
    }
    fn sides_open(&self, name: &str) -> bool {
        self.morphing.sides.contains(name)
    }
    fn toggle_morph_keys(&mut self, morphs: &[String]) {
        let Some((id, _, take)) = self.chosen_morphing() else {
            return;
        };
        if self.transport.is_playing() {
            self.transport.pause();
        }
        let frame = self.transport.frame();
        let mut pending = self.pending_morphs(id);
        let all_keyed = morphs
            .iter()
            .all(|morph| !pending.holds(morph) && take.morphs.key(morph, frame).is_some());
        if all_keyed {
            let ids: FxHashSet<KeyId> = morphs
                .iter()
                .map(|morph| KeyId::new(Lane::Morph(morph.clone()), frame))
                .collect();
            self.commit(Subject::Figure(id), Change::deleting(&take, &ids));
            return;
        }
        let keys = morphs
            .iter()
            .map(|morph| {
                let weight = match pending.holds(morph) {
                    true => pending.weight(morph),
                    false => take.morph_weight(morph, frame as f32),
                };
                self.morph_key(&take, morph, frame, weight)
            })
            .collect();
        self.lay(Subject::Figure(id), keys);
        for morph in morphs {
            pending.forget(morph);
        }
        self.stage.lock().hold_morphs(id, frame, pending);
    }
    fn toggle_sides(&mut self, name: &str) {
        if !self.morphing.sides.remove(name) {
            self.morphing.sides.insert(name.to_owned());
        }
    }
    fn type_morphs(&mut self, morphs: Vec<String>, weight: f32) {
        self.morphing.entry.set_text(format!("{weight:.2}"));
        self.morphing.entry.select_all();
        self.morphing.typing = Some(morphs);
    }
    fn typed_morphs(&self) -> Option<&[String]> {
        self.morphing.typing.as_deref()
    }
    fn unregistered_morphs(&self) -> Vec<String> {
        let Some(Subject::Figure(id)) = self.subject else {
            return Vec::new();
        };
        self.pending_morphs(id)
            .iter()
            .map(|(morph, _)| morph.to_owned())
            .collect()
    }
}

impl Animate {
    pub(crate) fn express(&mut self, change: impl FnOnce(&Faces, &mut MorphWeights)) {
        let Some((id, faces, take)) = self.chosen_morphing() else {
            return;
        };
        if self.transport.is_playing() {
            self.transport.pause();
        }
        let frame = self.transport.frame() as f32;
        let mut edited = take.morph_weights(frame);
        edited.overlay(&self.pending_morphs(id));
        change(&faces, &mut edited);
        let pending: MorphWeights = edited
            .iter()
            .filter(|(morph, weight)| (weight - take.morph_weight(morph, frame)).abs() > NOTICED)
            .map(|(morph, weight)| (morph.to_owned(), weight))
            .collect();
        self.stage
            .lock()
            .hold_morphs(id, self.transport.frame(), pending);
    }
}
