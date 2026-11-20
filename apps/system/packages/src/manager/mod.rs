mod backend;
mod index;
mod listing_view;
mod search;
mod view;

use std::collections::HashMap;
use std::sync::Arc;

use nixpkgs_search::{
    Catalog, CatalogCache, Nixpkgs, Options, Package, Pin, Requirement, Stage, Suggestion,
};
use rikka_app::prelude::*;
use rikka_nix::{Action, Channel, Generation, Modules, Progress, Request};

pub(crate) use self::view::{caption_bar, view};

use self::backend::Backend;
use self::index::Index;
use self::listing_view::ListingView;
use self::search::Search;
use crate::cancel_guard::{CancelGuard, unless_cancelled};
use crate::configuration::Declared;
use crate::error::Error;
use crate::job::Job;
use crate::listing::{Listing, Marks, Row, Rows};
use crate::plan::Plan;
use crate::platform;
use crate::section::Section;
use crate::work::Work;

const SEARCHING: &str = "packages.search.finding";

struct Offering {
    package: String,
    suggestions: Vec<Suggestion>,
    installer: Option<String>,
}

type LookupResult = Option<nixpkgs_search::Result<Vec<Package>>>;

#[derive(State)]
pub(crate) struct Manager {
    sender: Sender<Manager>,
    backend: Backend,
    opening: CancelGuard,
    index: Index,
    cataloging: CancelGuard,
    pinned: Option<Pin>,
    search: Search,
    search_focused: bool,
    section: Section,
    options: Option<Options>,
    reading_options: CancelGuard,
    declared: Option<Vec<String>>,
    modules: Option<Modules>,
    plan: Plan,
    listing_view: ListingView,
    generations: Vec<Generation>,
    work: Work,
    error: Option<Error>,
}

impl Manager {
    pub(crate) fn new(sender: Sender<Manager>) -> Self {
        Self {
            sender,
            backend: Backend::default(),
            opening: CancelGuard::default(),
            index: Index::default(),
            cataloging: CancelGuard::default(),
            pinned: None,
            search: Search::new(),
            search_focused: false,
            section: Section::default(),
            options: None,
            reading_options: CancelGuard::default(),
            declared: None,
            modules: None,
            plan: Plan::new(rikka_nix::login()),
            listing_view: ListingView::default(),
            generations: Vec::new(),
            work: Work::new(),
            error: None,
        }
    }

    pub(crate) fn start(&mut self) {
        self.open_index();
        self.open();
        self.read_options();
        self.read_declared();
        self.read_generations();
    }

    fn open_index(&mut self) {
        self.search.reset();
        let catalog_cache = match platform::catalog_cache() {
            Ok(folder) => CatalogCache::new(folder),
            Err(error) => {
                self.index = Index::Failed(error.into());
                return;
            }
        };
        let pin = platform::pin();
        self.pinned = Some(pin.clone());
        let cancel = self.cataloging.renew();
        self.index = Index::default();
        let sender = self.sender.clone();
        spawn_blocking(
            move || {
                catalog_cache.open(&pin, &cancel, move |stage| {
                    sender
                        .send(move |manager: &mut Manager| manager.stage_changed(stage))
                        .ok();
                })
            },
            |manager: &mut Manager, opened: nixpkgs_search::Result<Catalog>| {
                manager.catalog_opened(opened);
            },
        );
    }

    fn stage_changed(&mut self, stage: Stage) {
        if !self.index.advance(stage) {
            skip_view();
        }
    }

    fn catalog_opened(&mut self, opened: nixpkgs_search::Result<Catalog>) {
        let Some(opened) = unless_cancelled(opened) else {
            skip_view();
            return;
        };
        match opened {
            Ok(catalog) => {
                self.index = Index::Ready(Arc::new(catalog));
                self.find();
                let listed = self.listed();
                self.offer(&listed);
            }
            Err(error) => self.index = Index::Failed(error.into()),
        }
        self.describe();
        self.refresh_listing();
    }

    fn open(&mut self) {
        if self.backend.is_opening() {
            return;
        }
        let cancel = self.opening.renew();
        self.backend = Backend::Opening;
        spawn_blocking(
            move || {
                let channel = Channel::detect();
                Nixpkgs::open(platform::SYSTEM, &cancel).map(|nixpkgs| (channel, nixpkgs))
            },
            |manager: &mut Manager, opened: nixpkgs_search::Result<(Channel, Nixpkgs)>| {
                manager.backend = Backend::opened(opened);
                manager.describe();
            },
        );
    }

    fn read_declared(&self) {
        spawn_blocking(
            Declared::read,
            |manager: &mut Manager, declared: rikka_nix::Result<Declared>| match declared {
                Ok(declared) => manager.declared(declared),
                Err(error) => {
                    manager.error = Some(error.into());
                    manager.refresh_listing();
                }
            },
        );
    }

    fn declared(&mut self, declared: Declared) {
        let integrated = self
            .plan
            .integration_mut()
            .declared(&declared.integration, declared.unfree);
        if let Err(error) = integrated {
            self.error = Some(error.into());
            self.refresh_listing();
            return;
        }
        self.plan
            .apps_mut()
            .declared(declared.distribution, declared.apps);
        self.declared = Some(declared.packages);
        self.modules = Some(declared.modules);
        self.merge_declared();
    }

    fn read_options(&mut self) {
        let cancel = self.reading_options.renew();
        spawn_blocking(
            move || Options::open(platform::SYSTEM, &cancel),
            |manager: &mut Manager, read: nixpkgs_search::Result<Options>| match read {
                Ok(options) => {
                    manager.options = Some(options);
                    manager.merge_declared();
                }
                Err(error) => manager.error = Some(error.into()),
            },
        );
    }

    fn merge_declared(&mut self) {
        let Some(packages) = &self.declared else {
            return;
        };
        let mut declared = packages.clone();
        if let Some(options) = &self.options {
            let installed: Vec<String> = self
                .plan
                .integration()
                .enabled()
                .filter_map(|path| options.installed_package(path))
                .map(str::to_owned)
                .collect();
            declared.extend(installed);
        }

        self.plan.declared(declared);
        let listed = self.listed();
        self.offer(&listed);
        self.refresh_listing();
        self.describe();
    }

    fn listed(&self) -> Vec<String> {
        self.plan.listed().into_iter().map(str::to_owned).collect()
    }

    fn offer(&mut self, packages: &[String]) {
        for offering in self.offerings(packages) {
            let mark = self.plan.mark(&offering.package);
            self.plan.integration_mut().offer(
                &offering.package,
                offering.suggestions,
                offering.installer,
                mark,
            );
        }
    }

    fn offerings(&self, packages: &[String]) -> Vec<Offering> {
        let Some(options) = &self.options else {
            return Vec::new();
        };
        packages
            .iter()
            .map(|package| {
                let suggestions = options.requirements(package, self.unfree(package));
                let installer =
                    suggestions
                        .iter()
                        .find_map(|suggestion| match &suggestion.requirement {
                            Requirement::Option { path, .. }
                                if options.installed_package(path) == Some(package.as_str()) =>
                            {
                                Some(path.clone())
                            }
                            _ => None,
                        });
                Offering {
                    package: package.clone(),
                    suggestions,
                    installer,
                }
            })
            .collect()
    }

    fn unfree(&self, attribute: &str) -> bool {
        let cataloged = self
            .index
            .catalog()
            .and_then(|catalog| Some(catalog.entry(catalog.find(attribute)?).unfree));
        cataloged
            .or_else(|| {
                self.search
                    .description(attribute)
                    .map(|package| package.unfree)
            })
            .unwrap_or_default()
    }

    fn read_generations(&self) {
        spawn_blocking(
            Generation::all,
            |manager: &mut Manager, listed: rikka_nix::Result<Vec<Generation>>| match listed {
                Ok(generations) => manager.generations = generations,
                Err(error) => manager.error = Some(error.into()),
            },
        );
    }

    fn describe(&mut self) {
        if !self.index.is_finished() {
            return;
        }
        let Some(nixpkgs) = self.backend.nixpkgs().cloned() else {
            return;
        };
        let catalog = self.index.catalog();
        let unlisted: Vec<String> = self
            .listed()
            .into_iter()
            .filter(|attribute| catalog.is_none_or(|catalog| catalog.find(attribute).is_none()))
            .collect();
        let Some(cancel) = self.search.describe(unlisted.clone()) else {
            return;
        };
        spawn_blocking(
            move || {
                let attributes: Vec<&str> = unlisted.iter().map(String::as_str).collect();
                unless_cancelled(nixpkgs.lookup(&attributes, &cancel))
            },
            |manager: &mut Manager, described: LookupResult| manager.lookup_finished(described),
        );
    }

    fn lookup_finished(&mut self, described: LookupResult) {
        let Some(described) = described else {
            skip_view();
            return;
        };
        if let Err(error) = self.search.described(described) {
            self.error = Some(error.into());
        }
        let described = self.search.described_attributes();
        self.offer(&described);
        self.refresh_listing();
    }

    fn query_changed(&mut self) {
        self.error = None;
        self.find();
    }

    fn find(&mut self) {
        let Some(catalog) = self.index.catalog().cloned() else {
            return;
        };
        let Some(text) = self.search.query_text() else {
            cancel(SEARCHING);
            self.search.show_all(&catalog);
            self.refresh_listing();
            return;
        };
        spawn_blocking_keyed(
            SEARCHING,
            move || search::found(&catalog, &text),
            |manager: &mut Manager, found: Rows| {
                manager.search.set_found(found);
                manager.refresh_listing();
            },
        );
    }

    fn refresh_listing(&mut self) {
        let rows = match self.section {
            Section::Search => Arc::clone(self.search.found()),
            Section::Installed => Arc::new(self.installed()),
            Section::Apps | Section::Generations => Rows::default(),
        };
        let listing = Listing::new(self.index.catalog().cloned(), rows, self.marks());
        if let Err(error) = self.listing_view.set_listing(listing) {
            self.error = Some(error.into());
        }
    }

    fn installed(&self) -> Vec<Row> {
        let catalog = self.index.catalog();
        self.plan
            .listed()
            .into_iter()
            .map(|attribute| {
                if let Some(id) = catalog.and_then(|catalog| catalog.find(attribute)) {
                    return Row::Cataloged(id);
                }
                match self.search.description(attribute) {
                    Some(package) => Row::Described(Arc::clone(package)),
                    None => Row::Unknown(Arc::from(attribute)),
                }
            })
            .collect()
    }

    fn marks(&self) -> Marks {
        let marks: HashMap<String, _> = self
            .plan
            .listed()
            .into_iter()
            .map(|attribute| (attribute.to_owned(), self.plan.mark(attribute)))
            .collect();
        Arc::new(marks)
    }

    pub(crate) fn select(&mut self, index: usize) {
        match self.listing_view.select(index) {
            Ok(true) => {}
            Ok(false) => skip_view(),
            Err(error) => self.error = Some(error.into()),
        }
    }

    pub(crate) fn focus_search(&mut self) {
        if self.section != Section::Search {
            self.open_section(Section::Search);
        }
        self.search.query().select_all();
        self.search_focused = true;
    }

    fn search_blurred(&mut self) {
        if !self.search_focused {
            skip_view();
            return;
        }
        self.search_focused = false;
    }

    fn open_section(&mut self, section: Section) {
        if self.section == section {
            skip_view();
            return;
        }
        self.section = section;
        self.listing_view.deselect();
        self.refresh_listing();
        if section == Section::Installed && !self.search.looked_up() {
            self.describe();
        }
    }

    pub(crate) fn include(&mut self, attribute: &str) {
        self.plan.include(attribute);
        self.offer(&[attribute.to_owned()]);
        self.refresh_listing();
    }

    pub(crate) fn exclude(&mut self, attribute: &str) {
        self.plan.exclude(attribute);
        self.refresh_listing();
    }

    pub(crate) fn revert(&mut self, attribute: &str) {
        self.plan.revert(attribute);
        self.refresh_listing();
    }

    pub(crate) fn choose_app(&mut self, id: &str, on: bool) {
        self.plan.apps_mut().choose(id, on);
    }

    pub(crate) fn discard(&mut self) {
        self.plan.discard();
        self.refresh_listing();
    }

    pub(crate) fn apply(&mut self) {
        if self.work.is_busy() || self.plan.is_unchanged() {
            return;
        }
        let Some(declared) = self.modules.clone() else {
            return;
        };
        self.work.start(Job::Apply);
        match self.plan.changes().request(declared) {
            Ok(request) => self.run(request),
            Err(error) => self.work.failed(&error),
        }
    }

    fn update_system(&mut self) {
        self.begin(
            Job::Update,
            Request::Update {
                action: Action::Switch,
            },
        );
    }

    pub(crate) fn roll_back(&mut self, generation: u32) {
        self.begin(Job::Rollback(generation), Request::Rollback { generation });
    }

    fn dismiss_refusal(&mut self) {
        self.work.dismiss();
    }

    fn retry(&mut self) {
        self.error = None;
        if self.backend.unavailable().is_some() {
            self.open();
        }
        if matches!(self.index, Index::Failed(_)) {
            self.open_index();
        }
    }

    fn begin(&mut self, job: Job, request: Request) {
        if self.work.is_busy() {
            return;
        }
        self.work.start(job);
        self.run(request);
    }

    fn run(&mut self, request: Request) {
        let pending_request = self.work.begin(request);
        let sender = self.sender.clone();
        spawn_blocking(
            move || {
                pending_request.send(move |progress| {
                    sender
                        .send(move |manager: &mut Manager| manager.progressed(progress))
                        .ok();
                })
            },
            |manager: &mut Manager, sent: rikka_nix::Result<()>| {
                if let Err(error) = sent {
                    manager.work.failed(&error);
                }
            },
        );
    }

    fn progressed(&mut self, progress: Progress) {
        if self.work.progress(progress) {
            self.read_declared();
            self.read_generations();
            if self.pinned.as_ref() != Some(&platform::pin()) {
                self.open_index();
                self.refresh_listing();
            }
        }
    }
}

impl std::fmt::Debug for Manager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Manager")
            .field("section", &self.section)
            .field("index", &self.index)
            .field("listing", &self.listing_view.listing().len())
            .field("selected", &self.listing_view.selected())
            .field("plan", &self.plan)
            .field("work", &self.work)
            .finish_non_exhaustive()
    }
}
