use std::collections::BTreeSet;
use std::path::PathBuf;

use ade_watch::Sink;
use rikka_app::prelude::*;

use crate::ade::Ade;
use crate::project::state::Project;
use crate::setup::Setup;

#[derive(Clone)]
pub(crate) struct ProjectHandle {
    sender: Sender<Ade>,
}

impl ProjectHandle {
    pub(crate) fn new(sender: Sender<Ade>) -> ProjectHandle {
        ProjectHandle { sender }
    }

    pub(crate) fn say(&self, act: impl FnOnce(&mut Project) + Send + 'static) {
        let _sent = self.sender.send_to(move |project: &mut Project| {
            act(project);
            project.settle();
        });
    }

    pub(crate) fn reach(&self, act: impl FnOnce(&mut Project) + Send + 'static) {
        let _sent = self.sender.send_to(act);
    }

    pub(crate) fn say_with_setup(
        &self,
        act: impl FnOnce(&mut Project, &mut Setup) + Send + 'static,
    ) {
        let _sent = self.sender.send(move |ade: &mut Ade| {
            ade.work(act);
        });
    }

    pub(crate) fn sink(
        &self,
        stirred: impl Fn(&mut Project, BTreeSet<PathBuf>) + Clone + Send + Sync + 'static,
    ) -> Sink {
        let handle = self.clone();
        std::sync::Arc::new(move |folders| {
            let stirred = stirred.clone();
            handle.say(move |project| stirred(project, folders));
        })
    }
}

impl std::fmt::Debug for ProjectHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectHandle").finish_non_exhaustive()
    }
}
