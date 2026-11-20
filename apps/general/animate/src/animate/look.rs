use std::sync::Arc;

use animate_cast::{LookTrack, Subject};
use animate_edit::{Change, Key, KeyKind};
use animate_look::{
    Error, GraphRef, Look, LookEdit, LookKey, cinematic_composite, cinematic_world,
    default_composite, default_world, shared_library,
};
use animate_shader::{Job, Merge, Outcome, Synced};
use rikka_app::prelude::*;
use rikka_shader_graph::{Graph, Library};

use crate::animate::edit::{Edit, Pair};
use crate::animate::subject_motion::SubjectMotion;
use crate::animate::{Animate, live_viewport};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Starter {
    Bare,
    Cinematic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LookPart {
    World,
    Composite,
}

impl LookPart {
    fn graph(self) -> GraphRef {
        match self {
            Self::World => GraphRef::World,
            Self::Composite => GraphRef::Composite,
        }
    }

    fn starter(self, starter: Starter, library: &Library) -> Result<Graph, Error> {
        match (self, starter) {
            (Self::World, Starter::Bare) => default_world(library),
            (Self::World, Starter::Cinematic) => cinematic_world(library),
            (Self::Composite, Starter::Bare) => default_composite(library),
            (Self::Composite, Starter::Cinematic) => cinematic_composite(library),
        }
    }
}

impl Animate {
    pub(crate) fn look(&self) -> &Arc<Look> {
        &self.cast.look().motion
    }

    pub(crate) fn change_look<T, E>(
        &mut self,
        merge: Option<Merge>,
        change: impl FnOnce(&mut Look, &Library) -> Result<T, E>,
    ) -> Result<T, E> {
        let before = Arc::clone(self.look());
        let mut after = Look::clone(&before);
        let value = change(&mut after, self.workbench.library())?;
        let after = Arc::new(after);
        self.install_look(Arc::clone(&after));
        let edit = Edit::Replaced(Pair::new(
            Some(SubjectMotion::Look(before)),
            Some(SubjectMotion::Look(after)),
        ));
        match merge {
            Some(Merge(gesture)) => {
                self.history
                    .record_gesture(gesture, Subject::Look, edit, Edit::merge)
            }
            None => self.record(Subject::Look, edit),
        }
        Ok(value)
    }

    pub(crate) fn edit_look(&mut self, edit: LookEdit, merge: Option<Merge>) -> Result<(), Error> {
        self.change_look(merge, |look, library| look.apply(library, edit).map(drop))
    }

    pub(super) fn install_look(&mut self, look: Arc<Look>) {
        self.cast.set_look(LookTrack::new(Arc::clone(&look)));
        self.stage.lock().set_look(look);
        self.sync_shaders();
        self.follow_sampling();
        self.relength();
        if self.subject == Some(Subject::Look) {
            self.timeline
                .show(Some(Subject::Look), self.cast.sheet(Subject::Look));
        }
    }

    pub(crate) fn reset_look(&mut self, part: LookPart, starter: Starter) {
        let reset = part
            .starter(starter, self.workbench.library())
            .and_then(|graph| {
                self.edit_look(
                    LookEdit::Replace {
                        graph: part.graph(),
                        with: Arc::new(graph),
                    },
                    None,
                )
            });
        if let Err(error) = reset {
            self.notice = Some(Label::from(error.to_string()));
        }
    }

    pub(super) fn sync_shaders(&mut self) {
        let look = Arc::clone(self.look());
        for synced in self.workbench.sync(&look) {
            match synced {
                Synced::Changed(GraphRef::Material(key)) => self.request_compile(&look, &key),
                Synced::Renamed { from, to } => self.shaders.lock().rename(&from, &to),
                Synced::Changed(GraphRef::World | GraphRef::Composite) | Synced::Removed(_) => {}
            }
        }
    }

    fn request_compile(&mut self, look: &Look, key: &str) {
        let at = GraphRef::Material(key.to_owned());
        let (Some(revision), Some(graph)) =
            (self.workbench.graph_revision(&at), look.materials.get(key))
        else {
            return;
        };
        let job = self
            .shaders
            .lock()
            .request(key, revision, graph, self.workbench.library());
        if let Some(job) = job {
            self.run_shader_job(job);
        }
    }

    fn run_shader_job(&self, job: Job) {
        let compiler = self.compiler.clone();
        spawn_blocking(
            move || compiler.get().map(|compiler| job.run(&compiler)),
            |animate: &mut Animate, built: Result<Outcome, Arc<str>>| match built {
                Ok(outcome) => animate.shader_built(outcome),
                Err(error) => animate.notice = Some(Label::from(error.to_string())),
            },
        );
    }

    fn shader_built(&mut self, outcome: Outcome) {
        let (next, listeners) = {
            let mut shaders = self.shaders.lock();
            (shaders.finish(outcome), shaders.listeners())
        };
        for listener in listeners {
            listener();
        }
        if let Some(next) = next {
            self.run_shader_job(next);
        }
    }

    pub(super) fn follow_sampling(&mut self) {
        let time = self.transport.time();
        let evaluated = self.look_plans.evaluate(
            &self.cast.look().motion,
            shared_library(),
            self.transport.frame() as f32,
            time,
        );
        let Ok(evaluated) = evaluated else {
            return;
        };
        if evaluated.world.sampling == self.sampling {
            return;
        }
        self.sampling = evaluated.world.sampling;
        self.viewport
            .set_config(evaluated.world.configure(live_viewport()));
    }

    pub(crate) fn register_look(&mut self) {
        let frame = self.transport.frame();
        let look = Arc::clone(self.look());
        let library = self.workbench.library();
        let keys = look
            .keys
            .tracks()
            .tracks()
            .filter_map(|(target, held)| {
                Some(Key::Look(LookKey {
                    target: target.clone(),
                    frame,
                    value: look.value_at(library, target, frame as f32)?,
                    curve: held
                        .iter()
                        .find(|key| key.frame == frame)
                        .map_or(self.curves.value(KeyKind::Look), |key| key.curve),
                }))
            })
            .collect();
        self.commit(Subject::Look, Change::laying(&look, keys));
    }
}
