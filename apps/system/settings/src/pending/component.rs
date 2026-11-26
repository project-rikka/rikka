use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_nix::{
    Action, ChangeState, Document, Draft, Expr, FromNix, IntoAttrPath, Module, Modules, Outcome,
    Progress, Rebuild, Refusal, Request, Stage,
};

#[derive(State)]
pub(crate) struct Pending {
    unavailable: Option<String>,
    draft: Option<Draft>,
    changed: bool,
    error: Option<String>,
    overridden: Vec<Module>,
    rebuild: Rebuild,
    concluded: Option<Outcome>,
    report: Arc<dyn Fn(Progress) + Send + Sync>,
}

impl Pending {
    pub(crate) fn new(report: impl Fn(Progress) + Send + Sync + 'static) -> Self {
        let mut pending = Self {
            unavailable: None,
            draft: None,
            changed: false,
            error: None,
            overridden: Vec::new(),
            rebuild: Rebuild::new(),
            concluded: None,
            report: Arc::new(report),
        };
        pending.reload();
        pending
    }

    pub(crate) fn unavailable(&self) -> Option<&str> {
        self.unavailable.as_deref()
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn refusal(&self) -> Option<&Refusal> {
        self.rebuild.refusal()
    }

    pub(crate) fn overridden(&self) -> &[Module] {
        &self.overridden
    }

    pub(crate) fn changed(&self) -> bool {
        self.changed
    }

    pub(crate) fn busy(&self) -> bool {
        self.rebuild.is_running()
    }

    pub(crate) fn stage(&self) -> Stage {
        self.rebuild.stage()
    }

    pub(crate) fn concluded(&self) -> Option<&Outcome> {
        self.concluded.as_ref()
    }

    pub(crate) fn get<T: FromNix>(&self, module: Module, path: impl IntoAttrPath) -> Option<T> {
        self.edited(module)?.get(path).ok().flatten()
    }

    pub(crate) fn edited(&self, module: Module) -> Option<&Document> {
        self.draft.as_ref().map(|draft| draft.edited(module))
    }

    pub(crate) fn saved(&self, module: Module) -> Option<&Document> {
        self.draft.as_ref().map(|draft| draft.saved(module))
    }

    pub(crate) fn change(
        &mut self,
        module: Module,
        change: impl FnOnce(&mut Document) -> rikka_nix::Result<()>,
    ) {
        self.edit(module, |document| {
            let mut changed = document.clone();
            change(&mut changed)?;
            *document = changed;
            Ok::<(), rikka_nix::Error>(())
        });
    }

    pub(crate) fn set(&mut self, module: Module, path: impl IntoAttrPath, value: impl Into<Expr>) {
        let value = value.into();
        self.edit(module, |document| document.set(path, value));
    }

    pub(crate) fn remove(&mut self, module: Module, path: impl IntoAttrPath) {
        self.edit(module, |document| document.remove(path));
    }

    pub(crate) fn write(&mut self, module: Module, path: &str, text: Option<&str>) {
        match text {
            Some(text) => self.set(module, path, text),
            None => self.remove(module, path),
        }
    }

    pub(crate) fn discard(&mut self) {
        if let Some(draft) = &mut self.draft {
            draft.discard();
        }
        self.changed = false;
        self.error = None;
        self.overridden.clear();
        self.rebuild.dismiss_refusal();
    }

    pub(crate) fn dismiss_refusal(&mut self) {
        self.rebuild.dismiss_refusal();
    }

    pub(crate) fn dismiss_overridden(&mut self) {
        self.overridden.clear();
    }

    pub(crate) fn rollback(&mut self, generation: u32) {
        if self.rebuild.is_running() {
            return;
        }
        self.start(Request::Rollback { generation });
    }

    pub(crate) fn apply(&mut self) -> bool {
        if self.rebuild.is_running() || !self.changed() {
            return false;
        }
        if self.rebase() {
            return true;
        }
        if let Some(draft) = self.draft.as_ref().filter(|draft| draft.is_changed()) {
            let request = draft.request(Action::Switch);
            self.start(request);
        }
        false
    }

    fn start(&mut self, request: Request) {
        self.concluded = None;
        let pending_request = self.rebuild.start(request);
        let report = Arc::clone(&self.report);
        spawn_blocking(
            move || pending_request.send(move |progress| report(progress)),
            |pending: &mut Pending, sent: rikka_nix::Result<()>| {
                if let Err(error) = sent {
                    pending.rebuild.fail(&error);
                }
            },
        );
    }

    pub(crate) fn progress(&mut self, progress: Progress) -> bool {
        let Some(outcome) = self.rebuild.handle_progress(progress) else {
            return false;
        };
        self.conclude(&outcome);
        true
    }

    fn conclude(&mut self, outcome: &Outcome) {
        self.concluded = Some(outcome.clone());
        if matches!(outcome, Outcome::Refused(Refusal::Stale)) {
            self.rebase();
        } else if outcome.changes() == ChangeState::Modified {
            self.reload();
        }
    }

    fn reload(&mut self) {
        match Draft::system() {
            Ok(draft) => {
                self.unavailable = None;
                self.draft = Some(draft);
            }
            Err(error) => self.lose(&error),
        }
        self.changed = false;
        self.error = None;
        self.overridden.clear();
    }

    fn rebase(&mut self) -> bool {
        let Some(draft) = &mut self.draft else {
            return false;
        };
        match Modules::system() {
            Ok(current) => {
                self.overridden = draft.rebase(current);
                self.changed = draft.is_changed();
                !self.overridden.is_empty()
            }
            Err(error) => {
                self.lose(&error.into());
                self.changed = false;
                true
            }
        }
    }

    fn lose(&mut self, error: &rikka_nix::Error) {
        self.unavailable = Some(error.to_string());
        self.draft = None;
    }

    fn edit<E: std::fmt::Display>(
        &mut self,
        module: Module,
        edit: impl FnOnce(&mut Document) -> Result<(), E>,
    ) {
        if self.rebuild.is_running() {
            return;
        }
        let Some(draft) = &mut self.draft else {
            return;
        };
        self.error = edit(draft.edited_mut(module))
            .err()
            .map(|error| error.to_string());
        self.changed = draft.is_changed();
    }
}

impl std::fmt::Debug for Pending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pending")
            .field("changed", &self.changed())
            .field("stage", &self.rebuild.stage())
            .finish_non_exhaustive()
    }
}
