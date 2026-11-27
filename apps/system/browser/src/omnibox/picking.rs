use std::time::{Duration, SystemTime};

use browser_store::{ArchiveScope, Store};
use rikka_app::prelude::*;
use rikka_omnibox::{
    Action, Candidates, Classified, MatchKind, SUGGEST_DEBOUNCE, SearchEngine, SearchEngines,
    SearchSuggestion, Surface, classify,
};
use rikka_web::{History, HistoryQuery};

use crate::omnibox::gather;
use crate::omnibox::listing::Listing;
use crate::omnibox::lookup::{HistoryMatches, Lookup};
use crate::omnibox::picker::Picker;
use crate::omnibox::running::Opening;
use crate::omnibox::target::Target;
use crate::shell::Shell;
use browser_model::{ProfileId, ProfileKey};

const HISTORY: &str = "rikka-browser-omnibox-history";
const SUGGEST: &str = "rikka-browser-omnibox-suggest";
const HISTORY_LIMIT: u32 = 24;
const ARCHIVE_LIMIT: usize = 3;
const TOP_SITES: usize = 8;
const TOP_SITES_READ: u32 = 500;
const TOP_SITES_SPAN: Duration = Duration::from_secs(90 * 24 * 60 * 60);

type Suggested = rikka_omnibox::Result<(String, Vec<SearchSuggestion>)>;

impl Shell {
    pub(crate) fn picker(&self, surface: Surface) -> &Picker {
        match surface {
            Surface::AddressField => &self.address().omnibox.picker,
            Surface::CommandBar => &self.command_bar.picker,
        }
    }

    pub(crate) fn picker_mut(&mut self, surface: Surface) -> &mut Picker {
        match surface {
            Surface::AddressField => &mut self.address_mut().omnibox.picker,
            Surface::CommandBar => &mut self.command_bar.picker,
        }
    }

    pub(crate) fn input_of(&self, surface: Surface) -> String {
        match surface {
            Surface::AddressField => {
                let omnibox = &self.address().omnibox;
                match omnibox.chip() {
                    Some(chip) => format!("{} {}", chip.keyword, omnibox.typed),
                    None => omnibox.typed.clone(),
                }
            }
            Surface::CommandBar => self.command_bar.typed(),
        }
    }

    pub(crate) fn pick(&mut self, surface: Surface) -> Option<String> {
        let engines = self.browser().search_engines();
        let classified = classify(&self.input_of(surface), &engines);
        if surface == Surface::AddressField && classified.is_empty() {
            self.forget_picking(surface);
            return None;
        }
        self.ensure_lookup(surface);
        let picker = self.picker_mut(surface);
        let deleting = picker.deleting;
        let lookup = picker.lookup.as_mut()?;
        let inline = lookup.rank(&classified, &engines, deleting).inline.clone();
        let listing = {
            let model = self.model();
            let lookup = self.picker(surface).lookup.as_ref()?;
            Listing::new(surface, lookup.ranked(), &model, &engines)
        };
        self.picker_mut(surface).show(Some(listing));
        self.fetch_remote_suggestions(surface, &classified, &engines);
        inline
    }

    pub(crate) fn repick(&mut self, surface: Surface) {
        match surface {
            Surface::AddressField => self.pick_address(),
            Surface::CommandBar => {
                self.pick(surface);
            }
        }
    }

    pub(crate) fn forget_picking(&mut self, surface: Surface) {
        cancel((HISTORY, tag(surface)));
        cancel((SUGGEST, tag(surface)));
        self.picker_mut(surface).hide();
    }

    pub(crate) fn choose(&mut self, surface: Surface, index: usize, opening: Opening) {
        let selected_match = self
            .picker(surface)
            .lookup
            .as_ref()
            .and_then(|lookup| lookup.ranked().matches.get(index).cloned());
        let Some(selected_match) = selected_match else {
            return;
        };
        self.finish_picking(surface);
        self.run_match(selected_match, opening);
    }

    pub(crate) fn finish_picking(&mut self, surface: Surface) {
        self.forget_picking(surface);
        match surface {
            Surface::AddressField => self.leave_address(),
            Surface::CommandBar => self.command_bar.close(),
        }
    }

    pub(crate) fn hover_row(&mut self, surface: Surface, index: usize, inside: bool) {
        if let Some(listing) = self.picker_mut(surface).listing_mut() {
            let hovered = match inside {
                true => Some(index),
                false => listing.hovered.filter(|hovered| *hovered != index),
            };
            if listing.hovered != hovered {
                listing.hovered = hovered;
            }
        }
        skip_view();
    }

    pub(crate) fn remove_suggestion(&mut self, surface: Surface, index: usize) {
        let Some(profile) = self.persistent_profile() else {
            return;
        };
        let url = self.picker(surface).lookup.as_ref().and_then(|lookup| {
            let candidate = lookup.ranked().matches.get(index)?;
            match (&candidate.action, candidate.kind) {
                (Action::Navigate(url), MatchKind::History) => Some(url.to_string()),
                _ => None,
            }
        });
        let Some(url) = url else {
            return;
        };
        if let Some(runtime) = self
            .browser()
            .running_engine(ProfileKey::Persistent(profile))
        {
            spawn(
                runtime.history().delete_url(&url),
                |_: &mut Shell, deleted: rikka_web::Result<()>| {
                    if let Err(error) = deleted {
                        tracing::warn!(%error, "failed to remove a history entry");
                    }
                },
            );
        }
        if let Some(lookup) = self.picker_mut(surface).lookup.as_mut() {
            lookup.forget_url(&url);
        }
        self.repick(surface);
        skip_view();
    }

    fn ensure_lookup(&mut self, surface: Surface) {
        if self.picker(surface).lookup.is_some() {
            return;
        }
        let base = self.base_of(surface);
        let store = self.browser().store();
        let picker = self.picker_mut(surface);
        picker.lookup = Some(Lookup::new(surface, base));
        picker.store = Some(store);
    }

    fn base_of(&self, surface: Surface) -> Candidates<Target> {
        let model = self.model();
        let mut base = gather::open(&model, self.id());
        if surface == Surface::CommandBar {
            crate::command_bar::commands(self, &model, &mut base);
        }
        base
    }

    fn fetch_remote_suggestions(
        &mut self,
        surface: Surface,
        classified: &Classified,
        engines: &SearchEngines,
    ) {
        let text = classified.text().to_owned();
        let known = self
            .picker(surface)
            .lookup
            .as_ref()
            .is_some_and(|lookup| lookup.knows(&text));
        if let Some(profile) = self.persistent_profile()
            && !known
        {
            let store = self.browser().store();
            let history = self
                .browser()
                .running_engine(ProfileKey::Persistent(profile))
                .map(|runtime| runtime.history());
            spawn_keyed(
                (HISTORY, tag(surface)),
                find(history, store, profile, text.clone()),
                move |shell: &mut Shell, matches: HistoryMatches| {
                    shell.show_history(surface, &text, matches)
                },
            );
        }
        let on = self.browser().preferences().search_suggestions();
        match plan(on, classified, engines) {
            Some((engine, terms)) => after_keyed(
                (SUGGEST, tag(surface)),
                SUGGEST_DEBOUNCE,
                move |shell: &mut Shell| {
                    shell.fetch_suggestions(surface, engine.clone(), terms.clone())
                },
            ),
            None => cancel((SUGGEST, tag(surface))),
        }
    }

    fn show_history(&mut self, surface: Surface, input: &str, matches: HistoryMatches) {
        skip_view();
        let picker = self.picker_mut(surface);
        let Some(lookup) = picker.lookup.as_mut() else {
            return;
        };
        lookup.remember(input, matches);
        if picker.arrowed {
            return;
        }
        let engines = self.browser().search_engines();
        let current = classify(&self.input_of(surface), &engines);
        if current
            .text()
            .to_lowercase()
            .starts_with(&input.to_lowercase())
        {
            self.repick(surface);
        }
    }

    fn fetch_suggestions(&mut self, surface: Surface, engine: SearchEngine, terms: String) {
        skip_view();
        let Some(suggester) = self.browser().suggester() else {
            return;
        };
        spawn_keyed(
            (SUGGEST, tag(surface)),
            async move {
                let suggestions = suggester.suggest(&engine, &terms).await?;
                Ok((terms, suggestions))
            },
            move |shell: &mut Shell, suggested: Suggested| shell.suggested(surface, suggested),
        );
    }

    fn suggested(&mut self, surface: Surface, suggested: Suggested) {
        skip_view();
        let (terms, suggestions) = match suggested {
            Ok(suggested) => suggested,
            Err(error) => {
                tracing::debug!(%error, "search suggestions failed");
                return;
            }
        };
        let picker = self.picker_mut(surface);
        let Some(lookup) = picker.lookup.as_mut() else {
            return;
        };
        lookup.suggest(terms, suggestions);
        if !picker.arrowed {
            self.repick(surface);
        }
    }
}

fn tag(surface: Surface) -> u8 {
    match surface {
        Surface::AddressField => 0,
        Surface::CommandBar => 1,
    }
}

fn plan(
    on: bool,
    classified: &Classified,
    engines: &SearchEngines,
) -> Option<(SearchEngine, String)> {
    if !on {
        return None;
    }
    let terms = classified.suggestion_terms()?;
    let engine = match classified.search() {
        Some(search) => engines.get(search.engine())?,
        None => engines.default_engine(),
    };
    engine.suggestion_template()?;
    Some((engine.clone(), terms.to_owned()))
}

async fn find(
    history: Option<History>,
    store: Store,
    profile: ProfileId,
    typed: String,
) -> HistoryMatches {
    let query = match typed.is_empty() {
        true => HistoryQuery::new("")
            .since(
                SystemTime::now()
                    .checked_sub(TOP_SITES_SPAN)
                    .unwrap_or(SystemTime::UNIX_EPOCH),
            )
            .limit(TOP_SITES_READ),
        false => HistoryQuery::new(typed.clone()).limit(HISTORY_LIMIT),
    };
    let mut places = match history {
        Some(history) => history.search(&query).await.unwrap_or_else(|error| {
            tracing::warn!(%error, "failed to read history suggestions");
            Vec::new()
        }),
        None => Vec::new(),
    };
    if typed.is_empty() {
        places.sort_by_key(|place| std::cmp::Reverse(place.visit_count));
        places.truncate(TOP_SITES);
    }
    let archived = match typed.is_empty() {
        true => Vec::new(),
        false => store
            .search_archive(ArchiveScope::Profile(profile), &typed, ARCHIVE_LIMIT)
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "failed to search archive");
                Vec::new()
            }),
    };
    HistoryMatches { places, archived }
}
