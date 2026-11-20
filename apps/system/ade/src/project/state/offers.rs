use std::path::Path;
use std::sync::Arc;

use ade_paths::PathMatch;
use rikka_app::prelude::*;
use rikka_chat::{
    Attached, Builtin, DraftChange, OFFER_DWELL, Offer, Offering, argument_offers, invoked,
    prompt_offers, resource_offers,
};
use rikka_thread::Ask;

use crate::project::act::{act, given};
use crate::project::chat::AdeChat;
use crate::project::workspace::Workspace;

use super::Project;

const OFFERS: usize = 50;

impl Project {
    pub(in crate::project) fn unattach(&mut self, at: usize) {
        if at < self.attached.len() {
            self.attached.remove(at);
        }
    }

    pub(in crate::project) fn step(&mut self, down: bool) {
        if !self.offers.step(down) {
            skip_view();
        }
    }

    pub(in crate::project) fn point(&mut self, at: usize) {
        if !self.offers.point(at) {
            skip_view();
        }
    }

    pub(in crate::project) fn dismiss(&mut self) {
        self.offers.dismiss();
    }

    pub(in crate::project) fn attach_all(&mut self, attached: Attached) {
        if self.watching().is_some() || attached.is_empty() {
            skip_view();
            return;
        }
        let Attached {
            attachments,
            typed,
            refused,
        } = attached;
        self.attached.extend(attachments);
        if let Some(typed) = typed {
            self.draft.insert(&typed);
        }
        if let Some(reason) = refused {
            self.report_error(Arc::from(reason), None);
        }
    }

    pub(in crate::project) fn offer(&mut self) {
        let request = match self.offers.typed(&self.draft) {
            DraftChange::Command(query) => {
                let commanded = self.commanded(&query);
                self.offers.command(query, commanded);
                return;
            }
            DraftChange::Argument(request) => {
                after(OFFER_DWELL, act(move |project| project.complete(request)));
                return;
            }
            DraftChange::Mention(request) => request,
            DraftChange::Cleared | DraftChange::Nothing => return,
        };

        let root = self.shown_root();
        let Some(workspace) = self.workspaces.get(&root) else {
            return;
        };
        if workspace.is_stale()
            && let Some(workspace) = self.workspaces.get_mut(&root)
        {
            workspace.freshen();
        }
        after(
            OFFER_DWELL,
            act(move |project| project.look(&root, request)),
        );
    }

    fn commanded(&self, typed: &str) -> Vec<Offer> {
        let custom = self
            .shown()
            .commands()
            .matching(typed)
            .into_iter()
            .filter(|command| Builtin::named(command.name()).is_none())
            .map(|command| {
                Offer::command(
                    command.name(),
                    command.hint().map(Label::from),
                    Label::from(command.description()),
                )
            });
        Builtin::matching(typed)
            .map(Offer::builtin::<AdeChat>)
            .chain(custom)
            .chain(prompt_offers(&self.hub, typed))
            .take(OFFERS)
            .collect()
    }

    fn complete(&mut self, request: u64) {
        let Some(argument) = self.offers.pending_argument(request, &self.draft) else {
            skip_view();
            return;
        };
        let hub = self.hub.clone();
        spawn(
            argument_offers(hub, argument),
            given(move |project, offers: Vec<Offer>| project.found(request, offers)),
        );
    }

    fn look(&mut self, root: &Path, request: u64) {
        let pending_query = self
            .offers
            .pending_query(request, &self.draft)
            .filter(|_| root == self.shown_root());
        let Some(query) = pending_query else {
            skip_view();
            return;
        };
        let paths = self.workspaces.get(root).map(Workspace::paths);
        let hub = self.hub.clone();
        let root = root.to_owned();
        spawn_blocking(
            move || {
                let matches = paths.map_or_else(Vec::new, |paths| paths.matching(&query, OFFERS));
                matches
                    .iter()
                    .map(|path_match: &PathMatch| Offer::path(path_match.path()))
                    .chain(resource_offers(&hub, &query))
                    .collect()
            },
            given(
                move |project, offers: Vec<Offer>| match root == project.shown_root() {
                    true => project.found(request, offers),
                    false => skip_view(),
                },
            ),
        );
    }

    fn found(&mut self, request: u64, offers: Vec<Offer>) {
        if !self.offers.found(request, &self.draft, offers) {
            skip_view();
        }
    }

    pub(in crate::project) fn take(&mut self, at: usize) {
        let Some(offer) = self.offers.take(at, &self.draft) else {
            skip_view();
            return;
        };
        self.offer();
        if offer.runs {
            self.handle
                .say_with_setup(|project, setup| project.send(setup));
        }
    }

    pub(in crate::project) fn agree(&mut self) {
        if !self.offers.agree(&self.draft) {
            skip_view();
            return;
        }
        self.offer();
    }

    pub(in crate::project) fn offering(&self) -> Option<&Offering> {
        self.offers.offering()
    }

    pub(super) fn expanding(&self, text: String) -> Ask {
        let filled = invoked(&text).and_then(|invoked| {
            let command = self.shown().commands().get(invoked.name())?;
            Some(command.filled(invoked.arguments()).trim().to_owned())
        });
        match filled {
            Some(filled) => Ask::new(text).with_expansion(filled),
            None => Ask::new(text),
        }
    }
}
