use std::sync::Arc;

use animate_cast::Subject;
use animate_fluid::{Emitter, EmitterId, Pour, Shape};
use animate_inspector::EmitterNaming;
use animate_stage::Liquids;

use crate::animate::Animate;
use crate::animate::subject_motion::SubjectMotion;
use crate::wording;

impl Animate {
    pub(super) fn pour_liquids(&self) {
        let liquids = Liquids {
            emitters: self
                .cast
                .emitters()
                .iter()
                .map(|track| Arc::clone(&track.motion))
                .collect(),
            settings: self.pouring.settings,
        };
        let pouring = self.chosen_emitter().map(|emitter| emitter.id);
        let mut stage = self.stage.lock();
        stage.set_emitters(Arc::new(liquids));
        stage.pour_into(pouring);
    }

    pub(super) fn sync_naming(&mut self) {
        let chosen_name = self
            .chosen_emitter()
            .map(|emitter| (emitter.id, emitter.name.clone()));
        if self.pouring.named != chosen_name {
            if let Some((_, name)) = &chosen_name {
                self.pouring.naming.set_text(name);
            }
            self.pouring.named = chosen_name;
        }
    }

    pub(crate) fn add_liquid(&mut self) {
        let figure = match self.subject {
            Some(Subject::Figure(id)) => self.cast.figure(id),
            _ => self.cast.figures().first(),
        };
        let placing = self
            .stage
            .lock()
            .placing(figure.map(|figure| (figure.id, figure.model.bounds())));
        let id = self.cast.fresh_emitter();
        let mut emitter = Emitter::new(id, wording::liquid_name(id), Shape::Stream);
        emitter.pour = Pour::from_start(self.transport.frame());
        emitter.launch.offset = placing.launch;
        emitter.target.offset = placing.target;
        self.emit(id, Some(Arc::new(emitter)));
    }

    pub(crate) fn can_duplicate_liquid(&self) -> bool {
        self.chosen_emitter().is_some()
    }

    pub(crate) fn duplicate_liquid(&mut self) {
        let Some(source) = self.chosen_emitter() else {
            return;
        };
        let mut copy = (**source).clone();
        copy.name = wording::copy_name(&source.name);
        let id = self.cast.fresh_emitter();
        copy.id = id;
        self.emit(id, Some(Arc::new(copy)));
    }

    pub(crate) fn update_emitter(&mut self, change: impl FnOnce(&mut Emitter)) {
        let Some(id) = self.chosen_emitter().map(|emitter| emitter.id) else {
            return;
        };
        self.update::<Emitter>(Subject::Emitter(id), change);
    }

    pub(super) fn add_loaded_emitter(
        &mut self,
        mut emitter: Emitter,
        replacing: Option<EmitterId>,
    ) {
        emitter.id = match replacing.filter(|id| self.cast.emitter(*id).is_some()) {
            Some(id) => id,
            None => self.cast.fresh_emitter(),
        };
        self.emit(emitter.id, Some(Arc::new(emitter)));
    }

    fn emit(&mut self, id: EmitterId, emitter: Option<Arc<Emitter>>) {
        self.replace(Subject::Emitter(id), emitter.map(SubjectMotion::Emitter));
    }
}

impl EmitterNaming for Animate {
    fn chosen_emitter(&self) -> Option<&Arc<Emitter>> {
        let Some(Subject::Emitter(id)) = self.subject else {
            return None;
        };
        self.cast.emitter(id)
    }
    fn naming(&self) -> &rikka_app::prelude::TextDocument {
        &self.pouring.naming
    }
    fn rename_emitter(&mut self) {
        let typed = self.pouring.naming.text();
        let name = typed.trim();
        if name.is_empty() {
            self.pouring.named = None;
            self.sync_naming();
            return;
        }
        let name = name.to_owned();
        self.update_emitter(|emitter| emitter.name = name);
    }
}
