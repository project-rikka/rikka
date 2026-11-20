use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ade_paths::Paths;
use rikka_app::prelude::*;

use crate::project::act::given_with_setup;
use crate::project::assembly::Assembly;
use crate::project::catalogs::Catalogs;
use crate::project::workspace::Workspace;
use crate::setup::Setup;

use super::Project;

const SERVERS: &str = "ade-mcp";

impl Project {
    pub(super) fn follow_servers(&mut self) {
        self.instruction = self.hub.instruction();
        stream_keyed(
            SERVERS,
            self.hub.changes(),
            given_with_setup(|project, _: &mut Setup, (): ()| project.servers_changed()),
        );
        let hub = self.hub.clone();
        spawn(async move { hub.start().await }, |(): ()| skip_view());
    }

    fn servers_changed(&mut self) {
        let instruction = self.hub.instruction();
        if instruction != self.instruction {
            self.instruction = instruction;
            for workspace in self.workspaces.values_mut() {
                workspace.wither();
            }
        }

        let unreached: BTreeSet<String> = self
            .hub
            .statuses()
            .into_iter()
            .filter(|standing| standing.error().is_some())
            .map(|standing| standing.server().to_owned())
            .collect();
        let fresh: Vec<&str> = unreached
            .difference(&self.unreached)
            .map(String::as_str)
            .collect();
        if !fresh.is_empty() {
            let message = tr!("ade.mcp.unreached", servers = &fresh.join(", ")).into();
            self.report_error(message, None);
        }
        self.unreached = unreached;

        let troubles = self.hub.errors();
        if troubles != self.troubles {
            if !troubles.is_empty() {
                let message = tr!("ade.mcp.unreadable", reason = &troubles.join("; ")).into();
                self.report_error(message, None);
            }
            self.troubles = troubles;
        }
    }

    pub(in crate::project) fn stirred(&mut self, root: &Path, folders: BTreeSet<PathBuf>) {
        let Some(workspace) = self.workspaces.get_mut(root) else {
            skip_view();
            return;
        };
        let reread = folders.contains(root);
        workspace.changed(folders);
        if reread {
            recatalogue(root.to_owned());
        }
    }
    pub(in crate::project) fn walked(&mut self, root: &Path, paths: Arc<Paths>) {
        let Some(workspace) = self.workspaces.get_mut(root) else {
            skip_view();
            return;
        };
        workspace.settled(paths);
        match root == self.shown_root() {
            true => self.offer(),
            false => skip_view(),
        }
    }

    fn assembly(&self, setup: &Setup) -> Assembly {
        Assembly::new(self.hub.clone(), &setup.llm)
    }

    pub(super) fn refresh(&mut self, root: &Path, setup: &Setup) {
        let assembly = self.assembly(setup);
        let Some(workspace) = self.workspaces.get_mut(root) else {
            return;
        };
        if let Some(message) = workspace.reconsider(&assembly) {
            self.report_error(message, None);
        }
    }

    pub(super) fn catalogued(&mut self, root: &Path, catalogs: Catalogs, setup: &Setup) -> bool {
        let unchanged = self
            .workspaces
            .get(root)
            .is_none_or(|workspace| *workspace.catalogs() == catalogs);
        if unchanged {
            return false;
        }
        let assembly = self.assembly(setup);
        let Some(workspace) = self.workspaces.get_mut(root) else {
            return false;
        };
        if let Some(message) = workspace.recatalogue(catalogs, &assembly) {
            self.report_error(message, None);
        }
        true
    }

    pub(super) fn enter(&mut self, root: PathBuf, setup: &Setup) -> bool {
        if self.workspaces.contains_key(&root) {
            return false;
        }
        let (workspace, error_message) =
            Workspace::open(root.clone(), &self.assembly(setup), &self.handle);
        self.workspaces.insert(root.clone(), workspace);
        recatalogue(root);
        if let Some(message) = error_message {
            self.report_error(message, None);
        }
        true
    }

    pub(crate) fn reconsider(&mut self, setup: &Setup) {
        let assembly = self.assembly(setup);
        let troubles: Vec<Arc<str>> = self
            .workspaces
            .values_mut()
            .filter_map(|workspace| workspace.reconsider(&assembly))
            .collect();
        for message in troubles {
            self.report_error(message, None);
        }
    }
}

fn recatalogue(root: PathBuf) {
    let finding = root.clone();
    spawn_blocking(
        move || Catalogs::find(&finding),
        given_with_setup(move |project, setup, catalogs: Catalogs| {
            if !project.catalogued(&root, catalogs, setup) {
                skip_view();
            }
        }),
    );
}
