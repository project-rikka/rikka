use std::sync::Arc;

use rikka_fuzzy::{Haystack, Score, ranked};
use rikka_ui::{MaterialSymbol, tr};
use rikka_xdg::Catalog;
use shell_apps::{AppIcons, Apps, DesktopEntry, searchable};
use shell_kit::launch::Spawn;

use crate::arrivals::Arrivals;
use crate::provider::{Activation, Hit, Provider, ProviderInfo, Query};

pub(crate) const ID: &str = "apps";

#[derive(Debug)]
pub(super) struct AppsProvider {
    apps: Apps,
    arrivals: Arrivals,
    keyed: Option<Keyed>,
}

impl AppsProvider {
    #[must_use]
    pub(crate) fn new(apps: Apps, arrivals: Arrivals) -> AppsProvider {
        AppsProvider {
            apps,
            arrivals,
            keyed: None,
        }
    }
}

type Fields = Box<[(Haystack, f32)]>;

struct Keyed {
    catalogue: Arc<Catalog>,
    entries: Vec<Fields>,
    alphabetical: Box<[usize]>,
}

impl Keyed {
    fn from_catalogue(catalogue: Arc<Catalog>) -> Keyed {
        let entries = catalogue
            .listable()
            .map(|entry| {
                searchable(entry)
                    .map(|(field, weight)| (Haystack::new(field), weight))
                    .collect()
            })
            .collect();
        let mut named: Vec<(String, &DesktopEntry, usize)> = catalogue
            .listable()
            .enumerate()
            .map(|(at, entry)| (entry.name.to_lowercase(), entry, at))
            .collect();
        named.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.name.cmp(&b.1.name))
                .then_with(|| a.1.id.cmp(&b.1.id))
        });
        let alphabetical = named.into_iter().map(|(_, _, at)| at).collect();
        Keyed {
            catalogue,
            entries,
            alphabetical,
        }
    }

    fn cached<'a>(
        cached: &'a mut Option<Keyed>,
        catalogue: &Arc<Catalog>,
        arrivals: &Arrivals,
    ) -> &'a Keyed {
        if cached
            .as_ref()
            .is_some_and(|keyed| !Arc::ptr_eq(&keyed.catalogue, catalogue))
        {
            *cached = None;
        }
        cached.get_or_insert_with(|| {
            arrivals.observe(catalogue);
            Keyed::from_catalogue(Arc::clone(catalogue))
        })
    }
}

impl std::fmt::Debug for Keyed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keyed")
            .field("entries", &self.entries.len())
            .finish_non_exhaustive()
    }
}

fn row(icons: &AppIcons, entry: &DesktopEntry) -> Hit {
    let hit = Hit::new(entry.id.as_str(), entry.name.as_str()).icon(icons.icon_for(Some(entry)));
    match entry.description() {
        Some(description) => hit.subtitle(description),
        None => hit,
    }
}

impl Provider for AppsProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo::new(ID, tr!("shell.launcher.provider.applications"))
            .icon(MaterialSymbol::APPS)
    }

    fn reload(&mut self) {
        self.apps.installed().rescan_if_stale();
    }

    fn search(&mut self, query: &Query) -> Vec<Hit> {
        if query.is_empty() {
            return Vec::new();
        }
        let catalogue = self.apps.installed().current();
        let keyed = Keyed::cached(&mut self.keyed, &catalogue, &self.arrivals);
        let matches = catalogue
            .listable()
            .zip(&keyed.entries)
            .filter_map(|(entry, fields)| {
                let score = query.best_found(fields.iter())?;
                (score > Score::NEUTRAL).then_some((score, entry))
            });
        ranked(matches)
            .into_iter()
            .map(|(score, entry)| row(self.apps.icons(), entry).score(score))
            .collect()
    }

    fn listing(&mut self) -> Vec<Hit> {
        let catalogue = self.apps.installed().current();
        let keyed = Keyed::cached(&mut self.keyed, &catalogue, &self.arrivals);
        let listable: Vec<&DesktopEntry> = catalogue.listable().collect();
        let icons = self.apps.icons();
        self.arrivals.read(|arrivals| {
            keyed
                .alphabetical
                .iter()
                .map(|&at| {
                    let entry = listable[at];
                    let hit = row(icons, entry);
                    match arrivals.arrived(&entry.id) {
                        Some(arrival) => hit.arrived(arrival),
                        None => hit,
                    }
                })
                .collect()
        })
    }

    fn recall(&mut self, id: &str) -> Option<Hit> {
        let apps = &self.apps;
        apps.installed().with(|catalogue| {
            let entry = catalogue.listable().find(|entry| entry.id == id)?;
            Some(row(apps.icons(), entry))
        })
    }

    fn activate(&mut self, hit: &Hit) -> Activation {
        self.apps.installed().with(|catalogue| {
            let Some(entry) = catalogue.listable().find(|entry| entry.id == hit.id()) else {
                return Activation::Dismiss;
            };
            let Some(argv) = entry.command() else {
                tracing::warn!("{}: no program in `Exec={}`", entry.id, entry.exec);
                return Activation::Dismiss;
            };

            let mut spawn = Spawn::new(argv);
            spawn.cwd = entry.working_dir.clone();
            Activation::spawn(spawn)
        })
    }
}
