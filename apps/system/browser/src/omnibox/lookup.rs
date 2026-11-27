use std::collections::VecDeque;
use std::sync::Arc;

use browser_store::ArchivedTab;
use rikka_omnibox::{
    Candidates, Classified, Ranked, Request, SearchEngines, SearchSuggestion, Surface,
};
use rikka_web::HistoryItem;

use crate::omnibox::gather;
use crate::omnibox::target::Target;

const REMEMBERED: usize = 16;

#[derive(Debug, Default)]
pub(crate) struct HistoryMatches {
    pub(crate) places: Vec<HistoryItem>,
    pub(crate) archived: Vec<ArchivedTab>,
}

#[derive(Debug)]
pub(crate) struct Lookup {
    surface: Surface,
    base: Candidates<Target>,
    history_cache: VecDeque<(String, Arc<HistoryMatches>)>,
    suggested: Option<(String, Arc<[SearchSuggestion]>)>,
    ranked: Ranked<Target>,
}

impl Lookup {
    pub(crate) fn new(surface: Surface, base: Candidates<Target>) -> Lookup {
        Lookup {
            surface,
            base,
            history_cache: VecDeque::new(),
            suggested: None,
            ranked: Ranked {
                matches: Vec::new(),
                inline: None,
            },
        }
    }

    pub(crate) fn ranked(&self) -> &Ranked<Target> {
        &self.ranked
    }

    pub(crate) fn knows(&self, input: &str) -> bool {
        let input = input.to_lowercase();
        self.history_cache.iter().any(|(known, _)| *known == input)
    }

    pub(crate) fn remember(&mut self, input: &str, matches: HistoryMatches) {
        let input = input.to_lowercase();
        self.history_cache.retain(|(known, _)| *known != input);
        if self.history_cache.len() == REMEMBERED {
            self.history_cache.pop_front();
        }
        self.history_cache.push_back((input, Arc::new(matches)));
    }

    pub(crate) fn forget_url(&mut self, url: &str) {
        for (_, matches) in &mut self.history_cache {
            if matches.places.iter().any(|place| place.url == url) {
                let places = matches
                    .places
                    .iter()
                    .filter(|place| place.url != url)
                    .cloned()
                    .collect();
                *matches = Arc::new(HistoryMatches {
                    places,
                    archived: matches.archived.clone(),
                });
            }
        }
    }

    pub(crate) fn suggest(&mut self, terms: String, suggestions: Vec<SearchSuggestion>) {
        self.suggested = Some((terms, suggestions.into()));
    }

    pub(crate) fn rank(
        &mut self,
        input: &Classified,
        engines: &SearchEngines,
        just_deleted: bool,
    ) -> &Ranked<Target> {
        let mut candidates = self.base.clone();
        if let Some(matches) = self.history_for(input.text()) {
            gather::history(&mut candidates, &matches.places);
            gather::archived(&mut candidates, &matches.archived);
        }
        let suggestions = self.suggestions_for(input);
        let request = Request::new(input, engines)
            .surface(self.surface)
            .just_deleted(just_deleted)
            .suggestions(&suggestions);
        self.ranked = candidates.rank(&request);
        &self.ranked
    }

    fn history_for(&self, input: &str) -> Option<Arc<HistoryMatches>> {
        let input = input.to_lowercase();
        self.history_cache
            .iter()
            .filter(|(known, _)| input.starts_with(known.as_str()))
            .max_by_key(|(known, _)| known.len())
            .map(|(_, matches)| Arc::clone(matches))
    }

    fn suggestions_for(&self, input: &Classified) -> Arc<[SearchSuggestion]> {
        let (Some(terms), Some((suggested_for, suggestions))) =
            (input.suggestion_terms(), &self.suggested)
        else {
            return Arc::from([]);
        };
        let terms = terms.to_lowercase();
        match terms.starts_with(&suggested_for.to_lowercase()) {
            true => Arc::clone(suggestions),
            false => Arc::from([]),
        }
    }
}
