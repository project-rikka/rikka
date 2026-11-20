use std::collections::BTreeSet;
use std::path::PathBuf;

use rikka_dialog::{Chooser, ChooserResponse};
use rikka_mcp::{Catalog, Hub, Place, Server, ServerStatus};

use crate::prelude::*;
use crate::servers::draft::{Draft, Fault, Kind};
use crate::servers::offered::Offered;
use crate::servers::trial::Trial;

const FOLLOW: &str = "chat.servers.follow";
const TRIAL: &str = "chat.servers.trial";
const SIGNING: &str = "chat.servers.signing";

#[derive(State)]
pub struct Servers {
    hub: Hub,
    scope: usize,
    catalogs: Vec<Result<Catalog, String>>,
    standings: Vec<ServerStatus>,
    troubles: Vec<String>,
    drafts: Vec<Option<Draft>>,
    unfolded: BTreeSet<(String, Offered)>,
    menu: Option<String>,
    trouble: Option<String>,
}

impl Servers {
    #[must_use]
    pub fn new(hub: Hub) -> Servers {
        let places = hub.places().len();
        let mut servers = Servers {
            hub,
            scope: 0,
            catalogs: Vec::new(),
            standings: Vec::new(),
            troubles: Vec::new(),
            drafts: (0..places).map(|_| None).collect(),
            unfolded: BTreeSet::new(),
            menu: None,
            trouble: None,
        };
        servers.reread();
        servers
    }

    #[must_use]
    pub fn hub(&self) -> &Hub {
        &self.hub
    }

    pub fn follow(&mut self) {
        self.reread();
        stream_keyed(
            FOLLOW,
            self.hub.changes(),
            |servers: &mut Servers, (): ()| {
                servers.reread();
            },
        );
    }

    pub(crate) fn reread(&mut self) {
        self.catalogs = self
            .hub
            .places()
            .iter()
            .map(|place| {
                self.hub
                    .catalog(place.name())
                    .map_err(|error| error.to_string())
            })
            .collect();
        self.standings = self.hub.statuses();
        self.troubles = self.hub.errors();
    }

    pub(crate) fn places(&self) -> &[Place] {
        self.hub.places()
    }

    pub(crate) fn scope(&self) -> usize {
        self.scope
    }

    pub(crate) fn place(&self) -> &Place {
        &self.hub.places()[self.scope]
    }

    pub(crate) fn declared(&self) -> Result<&Catalog, &str> {
        self.declared_at(self.scope)
    }

    fn declared_at(&self, scope: usize) -> Result<&Catalog, &str> {
        match self.catalogs.get(scope) {
            Some(Ok(catalog)) => Ok(catalog),
            Some(Err(trouble)) => Err(trouble),
            None => Err(""),
        }
    }

    pub(crate) fn standing(&self, server: &str) -> Option<&ServerStatus> {
        self.standings
            .iter()
            .find(|standing| standing.server() == server)
    }

    pub(crate) fn troubles(&self) -> impl Iterator<Item = &str> {
        self.troubles
            .iter()
            .chain(&self.trouble)
            .map(String::as_str)
    }

    pub(crate) fn is_drafting(&self) -> bool {
        self.draft().is_some()
    }

    pub(crate) fn is_unfolded(&self, name: &str, offered: Offered) -> bool {
        self.unfolded.contains(&(name.to_owned(), offered))
    }

    fn fold(&mut self, name: &str) {
        self.unfolded.retain(|(unfolded, _)| unfolded != name);
    }

    pub(crate) fn shows_menu(&self, name: &str) -> bool {
        self.menu.as_deref() == Some(name)
    }

    pub(crate) fn is_signing_in(&self, name: &str) -> bool {
        self.hub.is_signing_in(name)
    }

    pub(crate) fn is_signed_in(&self, name: &str) -> bool {
        self.hub.is_signed_in(name)
    }

    pub(crate) fn carriable(&self, name: &str) -> Vec<usize> {
        (0..self.places().len())
            .filter(|at| *at != self.scope)
            .filter(|at| {
                self.declared_at(*at)
                    .is_ok_and(|catalog| catalog.get(name).is_none())
            })
            .collect()
    }

    pub(crate) fn draft(&self) -> Option<&Draft> {
        self.drafts.get(self.scope)?.as_ref()
    }

    pub(crate) fn checked(&self) -> Option<Result<Server, Fault>> {
        let draft = self.draft()?;
        Some(draft.server().and_then(|server| {
            let renamed = draft.was() != Some(server.name());
            let in_use = self
                .declared()
                .is_ok_and(|catalog| catalog.get(server.name()).is_some());
            match renamed && in_use {
                true => Err(Fault::Taken),
                false => Ok(server),
            }
        }))
    }

    pub(crate) fn look(&mut self, scope: usize) {
        if scope < self.places().len() {
            self.scope = scope;
        }
        self.menu = None;
    }

    pub(crate) fn add(&mut self) {
        self.menu = None;
        if let Some(slot) = self.drafts.get_mut(self.scope) {
            slot.get_or_insert_with(Draft::blank);
        }
    }

    pub(crate) fn edit(&mut self, name: &str) {
        self.menu = None;
        let Some(server) = self
            .declared()
            .ok()
            .and_then(|catalog| catalog.get(name))
            .cloned()
        else {
            return;
        };
        if let Some(slot) = self.drafts.get_mut(self.scope) {
            *slot = Some(Draft::from_server(&server));
        }
    }

    pub(crate) fn dismiss(&mut self) {
        if let Some(slot) = self.drafts.get_mut(self.scope) {
            *slot = None;
        }
        cancel((TRIAL, self.scope));
    }

    pub(crate) fn edited(&mut self) {
        self.alter(Draft::changed);
    }

    pub(crate) fn choose(&mut self, kind: Kind) {
        self.alter(|draft| draft.choose(kind));
    }

    pub(crate) fn add_pair(&mut self) {
        self.alter(Draft::add_pair);
    }

    pub(crate) fn drop_pair(&mut self, at: usize) {
        self.alter(|draft| draft.drop_pair(at));
    }

    pub(crate) fn toggle_pair(&mut self, at: usize) {
        self.alter(|draft| draft.toggle_pair(at));
    }

    pub(crate) fn toggle_secret(&mut self) {
        self.alter(Draft::toggle_secret);
    }

    pub(crate) fn unfold_advanced(&mut self) {
        self.alter(Draft::unfold_advanced);
    }

    pub(crate) fn browse(&mut self) {
        let scope = self.scope;
        let title = tr!("chat.servers.folder-choose").to_string();
        let accept = tr!("chat.servers.folder-accept").to_string();
        let parent = window_identifier();
        spawn_blocking(
            move || pick_folder(&title, &accept, parent),
            move |servers: &mut Servers, chosen_folder: Option<PathBuf>| match (
                chosen_folder,
                servers.drafts.get_mut(scope).and_then(Option::as_mut),
            ) {
                (Some(folder), Some(draft)) => draft.locate(&folder.display().to_string()),
                _ => skip_view(),
            },
        );
    }

    pub(crate) fn try_out(&mut self) {
        let scope = self.scope;
        let hub = self.hub.clone();
        let Some(draft) = self.drafts.get_mut(scope).and_then(Option::as_mut) else {
            return;
        };
        let Ok(server) = draft.server() else {
            return;
        };
        draft.tried(Trial::Trying(server.clone()));
        spawn_keyed(
            (TRIAL, scope),
            Trial::run(hub, server.clone()),
            move |servers: &mut Servers, trial: Trial| servers.tried(scope, &server, trial),
        );
    }

    fn tried(&mut self, scope: usize, server: &Server, trial: Trial) {
        let Some(draft) = self.drafts.get_mut(scope).and_then(Option::as_mut) else {
            skip_view();
            return;
        };
        match draft.trial() {
            Trial::Trying(tested) if tested == server => draft.tried(trial),
            _ => skip_view(),
        }
    }

    pub(crate) fn keep(&mut self) {
        let Some(Ok(server)) = self.checked() else {
            return;
        };
        let was = self.draft().and_then(Draft::was).map(str::to_owned);
        if let Some(was) = &was {
            self.fold(was);
        }
        if self.change(self.scope, |catalog| {
            if let Some(was) = &was {
                catalog.remove(was);
            }
            catalog.insert(server);
        }) {
            self.dismiss();
        }
    }

    pub(crate) fn enable(&mut self, name: &str, on: bool) {
        self.change(self.scope, |catalog| {
            if let Some(server) = catalog.get(name).cloned() {
                catalog.insert(server.with_disabled(!on));
            }
        });
    }

    pub(crate) fn remove(&mut self, name: &str) {
        self.menu = None;
        self.fold(name);
        self.change(self.scope, |catalog| {
            catalog.remove(name);
        });
    }

    pub(crate) fn carry(&mut self, name: &str, to: usize, moving: bool) {
        self.menu = None;
        if !self.carriable(name).contains(&to) {
            return;
        }
        let Some(server) = self
            .declared()
            .ok()
            .and_then(|catalog| catalog.get(name))
            .cloned()
        else {
            return;
        };
        if self.change(to, |there| there.insert(server)) && moving {
            self.remove(name);
        }
    }

    pub(crate) fn sign_in(&mut self, name: &str) {
        self.menu = None;
        let hub = self.hub.clone();
        let server = name.to_owned();
        spawn_keyed(
            (SIGNING, name),
            async move {
                hub.sign_in(&server)
                    .await
                    .map_err(|error| error.to_string())
            },
            |servers: &mut Servers, signed: Result<(), String>| servers.landed(signed),
        );
        self.reread();
    }

    pub(crate) fn sign_out(&mut self, name: &str) {
        self.menu = None;
        let hub = self.hub.clone();
        let server = name.to_owned();
        spawn(
            async move {
                hub.sign_out(&server)
                    .await
                    .map_err(|error| error.to_string())
            },
            |servers: &mut Servers, signed: Result<(), String>| servers.landed(signed),
        );
    }

    pub(crate) fn reconnect(&mut self, name: &str) {
        self.menu = None;
        let hub = self.hub.clone();
        let server = name.to_owned();
        spawn(
            async move {
                hub.reconnect(&server)
                    .await
                    .map_err(|error| error.to_string())
            },
            |servers: &mut Servers, done: Result<(), String>| servers.landed(done),
        );
    }

    pub(crate) fn unfold(&mut self, name: &str, offered: Offered) {
        let key = (name.to_owned(), offered);
        if !self.unfolded.remove(&key) {
            self.unfolded.insert(key);
        }
    }

    pub(crate) fn open_menu(&mut self, name: &str) {
        self.menu = Some(name.to_owned());
    }

    pub(crate) fn close_menu(&mut self) {
        self.menu = None;
    }

    fn alter(&mut self, change: impl FnOnce(&mut Draft)) {
        let scope = self.scope;
        let Some(draft) = self.drafts.get_mut(scope).and_then(Option::as_mut) else {
            return;
        };
        let trying = draft.trial().is_trying();
        change(draft);
        if trying && !draft.trial().is_trying() {
            cancel((TRIAL, scope));
        }
    }

    fn change(&mut self, scope: usize, edit: impl FnOnce(&mut Catalog)) -> bool {
        let Ok(mut catalog) = self.declared_at(scope).cloned() else {
            return false;
        };
        edit(&mut catalog);
        self.trouble = None;
        if let Some(slot) = self.catalogs.get_mut(scope) {
            *slot = Ok(catalog.clone());
        }

        let hub = self.hub.clone();
        let place = self.places()[scope].name().to_owned();
        spawn(
            async move {
                hub.save_catalog(&place, catalog)
                    .await
                    .map_err(|error| error.to_string())
            },
            |servers: &mut Servers, result: Result<(), String>| servers.landed(result),
        );
        true
    }

    fn landed(&mut self, done: Result<(), String>) {
        self.trouble = done.err();
        self.reread();
    }
}

fn pick_folder(title: &str, accept: &str, parent: Option<WindowIdentifier>) -> Option<PathBuf> {
    match Chooser::open(title)
        .parent(parent)
        .accept_label(accept)
        .directory(true)
        .run()
    {
        Ok(ChooserResponse::Files(picked)) => picked.into_iter().next(),
        Ok(ChooserResponse::Cancelled) => None,
        Err(error) => {
            tracing::warn!("folder picker failed: {error}");
            None
        }
    }
}

impl std::fmt::Debug for Servers {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Servers")
            .field("hub", &self.hub)
            .field("scope", &self.scope)
            .field("menu", &self.menu)
            .finish_non_exhaustive()
    }
}
