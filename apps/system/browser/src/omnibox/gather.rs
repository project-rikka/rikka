use std::time::SystemTime;

use browser_store::ArchivedTab;
use rikka_omnibox::{Candidate, Candidates};
use rikka_web::HistoryItem;

use crate::model::Model;
use crate::omnibox::target::Target;
use browser_model::{ProfileKey, Slot, WindowId};

pub(crate) fn open(model: &Model, window: WindowId) -> Candidates<Target> {
    let mut candidates = Candidates::new();
    let Some(layout) = model.window(window) else {
        return candidates;
    };
    let active = layout.active();
    for (_, tabs) in layout.todays() {
        for tab in tabs.iter().copied().filter(|tab| Some(*tab) != active) {
            let Some(tab_entry) = model.tab(tab) else {
                continue;
            };
            if tab_entry.is_blank() {
                continue;
            }
            let (url, title) = (tab_entry.address(), tab_entry.meta.title.as_str());
            push(
                &mut candidates,
                Candidate::open_tab(Target::Tab(tab), &url, title),
            );
        }
    }
    let ProfileKey::Persistent(profile) = model.profile_here(window) else {
        return candidates;
    };
    for space in model
        .spaces
        .list()
        .iter()
        .filter(|space| space.profile == profile)
    {
        for pin in model.pinned.pins(space.id) {
            let Some((_, pinned)) = model.pinned.pin(pin) else {
                continue;
            };
            let target = match layout.tab_in_slot(Slot::Pinned(pin)) {
                Some(tab) if Some(tab) == active => continue,
                Some(tab) => Target::Tab(tab),
                None => Target::Slot(Slot::Pinned(pin), Some(space.id)),
            };
            let saved = &pinned.saved;
            push(
                &mut candidates,
                Candidate::pinned(target, &saved.url, &saved.title),
            );
        }
    }
    for favorite in model.favorites.for_profile(profile) {
        let target = match layout.tab_in_slot(Slot::Favorite(favorite.id)) {
            Some(tab) if Some(tab) == active => continue,
            Some(tab) => Target::Tab(tab),
            None => Target::Slot(Slot::Favorite(favorite.id), None),
        };
        let saved = &favorite.saved;
        push(
            &mut candidates,
            Candidate::favorite(target, &saved.url, &saved.title),
        );
    }
    candidates
}

pub(crate) fn history(candidates: &mut Candidates<Target>, places: &[HistoryItem]) {
    for place in places {
        let mut candidate = Candidate::history(&place.url, &place.title)
            .visits(place.visit_count)
            .typed(place.typed_count);
        if let Some(at) = place.last_visit_time {
            candidate = candidate.last_visit(at);
        }
        push(candidates, candidate);
    }
}

pub(crate) fn archived(candidates: &mut Candidates<Target>, tabs: &[ArchivedTab]) {
    for tab in tabs {
        let candidate = Candidate::archived(Target::Archived(tab.id), &tab.url, &tab.title)
            .last_visit(SystemTime::from(tab.archived_at));
        push(candidates, candidate);
    }
}

fn push(candidates: &mut Candidates<Target>, candidate: Candidate<'_, Target>) {
    if let Err(error) = candidates.push(candidate) {
        tracing::debug!(%error, "skipping suggestion");
    }
}
