mod install;
mod project;

use std::collections::BTreeSet;
use std::rc::Rc;
use std::time::Duration;

use launcher_content::{loaders, project_type};
use launcher_instance::ContentKind;
use launcher_modrinth::{Category, SearchHit, SearchQuery, SearchResults, SortIndex};
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{column, row};

pub(crate) use install::{WorldPick, world_sheet};
pub(crate) use project::{ProjectPage, project_view};

use crate::format;
use crate::kinds::{KINDS, kind_label, kind_symbol};
use crate::launcher::Launcher;
use crate::load::Load;
use crate::pictures;
use crate::target;
use crate::widgets::{chip, labelled, stat};

const DEBOUNCE: Duration = Duration::from_millis(300);
const ROW: f32 = 112.0;
const ICON: f32 = 72.0;
const SHOWN_CATEGORIES: usize = 3;
const KIND_SWITCH: f32 = 560.0;
const SORT: f32 = 190.0;

fn sort_label(sort: SortIndex) -> Label {
    match sort {
        SortIndex::Relevance => tr!("launcher.discover.sort.relevance"),
        SortIndex::Downloads => tr!("launcher.discover.sort.downloads"),
        SortIndex::Follows => tr!("launcher.discover.sort.follows"),
        SortIndex::Newest => tr!("launcher.discover.sort.newest"),
        SortIndex::Updated => tr!("launcher.discover.sort.updated"),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Results {
    hits: Rc<[SearchHit]>,
    total: u32,
}

pub(crate) struct Discover {
    pub(crate) kind: ContentKind,
    query: TextDocument,
    sort: SortIndex,
    categories: BTreeSet<String>,
    results: Load<Results>,
    more: bool,
    catalog: Load<Rc<[Category]>>,
    pub(crate) target: Option<String>,
    pub(crate) installing: BTreeSet<String>,
}

impl Default for Discover {
    fn default() -> Discover {
        Discover {
            kind: ContentKind::Mod,
            query: TextDocument::new(""),
            sort: SortIndex::Relevance,
            categories: BTreeSet::new(),
            results: Load::Idle,
            more: false,
            catalog: Load::Idle,
            target: None,
            installing: BTreeSet::new(),
        }
    }
}

impl std::fmt::Debug for Discover {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Discover")
            .field("kind", &self.kind)
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

impl Discover {
    pub(crate) fn ensure_target(
        &mut self,
        instances: &[launcher_instance::Instance],
        selected: Option<&str>,
    ) {
        let valid = self
            .target
            .as_deref()
            .is_some_and(|target| instances.iter().any(|instance| instance.id == target));
        if !valid {
            self.target = selected
                .filter(|selected| instances.iter().any(|instance| instance.id == *selected))
                .map(str::to_owned)
                .or_else(|| instances.first().map(|instance| instance.id.clone()));
        }
    }

    pub(crate) fn browse_for(&mut self, instance: Option<String>, kind: ContentKind) {
        if instance.is_some() {
            self.target = instance;
        }
        if self.kind != kind {
            self.kind = kind;
            self.categories.clear();
            self.results = Load::Idle;
        }
    }
}

impl Launcher {
    pub(crate) fn start_discover(&mut self) {
        let Some(services) = self.services() else {
            return;
        };
        self.discover.catalog = Load::Loading;
        spawn(
            async move { services.catalog.modrinth().categories().await },
            |launcher: &mut Launcher, found: launcher_modrinth::Result<Vec<Category>>| {
                launcher.discover.catalog = match found {
                    Ok(found) => Load::Ready(found.into()),
                    Err(error) => Load::Failed(error.to_string()),
                };
            },
        );
        self.search(true);
    }

    pub(crate) fn search(&mut self, reset: bool) {
        let Some(services) = self.services() else {
            return;
        };
        let discover = &mut self.discover;
        let offset = match (reset, discover.results.ready()) {
            (false, Some(results)) => u32::try_from(results.hits.len()).unwrap_or(u32::MAX),
            _ => 0,
        };
        let mut query = SearchQuery::new(project_type(discover.kind));
        query.text = discover.query.text();
        query.categories = discover.categories.iter().cloned().collect();
        query.game_version = Some(target::rikka().minecraft);
        query.loaders = loaders(discover.kind);
        query.sort = discover.sort;
        query.offset = offset;
        match reset {
            true => discover.results = Load::Loading,
            false => discover.more = true,
        }
        spawn_keyed(
            "launcher-search",
            async move { services.catalog.modrinth().search(&query).await },
            move |launcher: &mut Launcher, found: launcher_modrinth::Result<SearchResults>| {
                let discover = &mut launcher.discover;
                discover.more = false;
                match found {
                    Ok(found) => {
                        let hits: Vec<SearchHit> = match (reset, discover.results.ready()) {
                            (false, Some(previous)) => {
                                previous.hits.iter().cloned().chain(found.hits).collect()
                            }
                            _ => found.hits,
                        };
                        discover.results = Load::Ready(Results {
                            hits: hits.into(),
                            total: found.total_hits,
                        });
                    }
                    Err(error) => match reset {
                        true => discover.results = Load::Failed(error.to_string()),
                        false => launcher.fail(error.to_string()),
                    },
                }
            },
        );
    }

    fn query_changed(&mut self) {
        after_keyed(
            "launcher-search-debounce",
            DEBOUNCE,
            |launcher: &mut Launcher| {
                launcher.search(true);
            },
        );
    }

    fn set_kind(&mut self, kind: ContentKind) {
        if self.discover.kind == kind {
            return;
        }
        self.discover.kind = kind;
        self.discover.categories.clear();
        self.search(true);
    }

    fn set_sort(&mut self, sort: SortIndex) {
        self.discover.sort = sort;
        self.search(true);
    }

    fn toggle_category(&mut self, name: String) {
        if !self.discover.categories.remove(&name) {
            self.discover.categories.insert(name);
        }
        self.search(true);
    }

    fn set_target(&mut self, index: usize) {
        self.discover.target = self
            .instances
            .get(index)
            .map(|instance| instance.id.clone());
    }

    pub(crate) fn installed_projects(&self) -> BTreeSet<String> {
        self.discover
            .target
            .as_deref()
            .and_then(|id| self.instance(id))
            .map(|instance| {
                instance
                    .config
                    .content
                    .values()
                    .filter_map(|record| record.project_id.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InstallState {
    Available,
    Installing,
    Installed,
    NoTarget,
}

pub(crate) fn install_state(
    launcher: &Launcher,
    installed: &BTreeSet<String>,
    project: &str,
) -> InstallState {
    if launcher.discover.target.is_none() {
        InstallState::NoTarget
    } else if launcher.discover.installing.contains(project) {
        InstallState::Installing
    } else if installed.contains(project) {
        InstallState::Installed
    } else {
        InstallState::Available
    }
}

pub(crate) fn install_button(
    state: InstallState,
    project: String,
    kind: ContentKind,
    version: Option<String>,
) -> Element {
    match state {
        InstallState::Installed => button(labelled(
            MaterialSymbol::CHECK,
            tr!("launcher.discover.installed"),
        ))
        .secondary()
        .enabled(false)
        .any(),
        InstallState::Installing => button(text(tr!("launcher.discover.installing")))
            .secondary()
            .loading(true)
            .any(),
        InstallState::NoTarget | InstallState::Available => button(labelled(
            MaterialSymbol::DOWNLOAD,
            tr!("launcher.discover.install"),
        ))
        .primary()
        .enabled(state == InstallState::Available)
        .on_press(move |launcher: &mut Launcher| {
            launcher.install(project.clone(), kind, version.clone());
        })
        .any(),
    }
}

fn hit_card(hit: &SearchHit, kind: ContentKind, state: InstallState) -> Element {
    let opened = hit.project_id.clone();
    let updated = format::iso_ago(&hit.date_modified);
    container(
        row! {
            pictures::remote(hit.icon_url.as_deref(), kind_symbol(kind), ICON, 14.0);
            column! {
                row! {
                    text(hit.title.clone()).label().semibold().no_wrap().min_width(0.0);
                    text(tr!("launcher.discover.by", author = hit.author.as_str()))
                        .caption()
                        .muted()
                        .no_wrap();
                }
                .gap(Space::SNUG)
                .align_items(Align::Center);
                text(hit.description.clone()).caption().muted().fill_width();
                row! {
                    stat(MaterialSymbol::DOWNLOAD, format::count(hit.downloads));
                    stat(MaterialSymbol::FAVORITE, format::count(hit.follows));
                    updated.map(|updated| stat(MaterialSymbol::UPDATE, updated));
                    row_of(
                        hit.display_categories
                            .iter()
                            .take(SHOWN_CATEGORIES)
                            .map(|category| chip(category.clone(), palette().text_muted)),
                    )
                    .gap(Space::TIGHT);
                }
                .gap(Space::NORMAL)
                .align_items(Align::Center);
            }
            .gap(Space::TIGHT)
            .grow(1.0)
            .min_width(0.0);
            install_button(state, hit.project_id.clone(), kind, None);
        }
        .gap(Space::ROOMY)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::all(Space::NORMAL))
    .surface()
    .radius(Corner::Panel)
    .border(1.0, palette().outline)
    .on_press(move |launcher: &mut Launcher| launcher.open_project(opened.clone()))
    .fill_width()
    .any()
}

fn categories(launcher: &Launcher) -> Option<Element> {
    let catalog = launcher.discover.catalog.ready()?;
    let kind = project_type(launcher.discover.kind);
    let shown: Vec<(String, bool)> = catalog
        .iter()
        .filter(|category| category.project_type == kind && category.header == "categories")
        .map(|category| {
            (
                category.name.clone(),
                launcher.discover.categories.contains(&category.name),
            )
        })
        .collect();
    if shown.is_empty() {
        return None;
    }
    Some(
        row_of(shown.iter().map(|(name, chosen)| {
            let toggled = name.clone();
            let (fill, ink, edge) = match chosen {
                true => (palette().accent, palette().on_accent, palette().accent),
                false => (Color::TRANSPARENT, palette().text, palette().outline),
            };
            container(text(name.clone()).caption().medium().color(ink).no_wrap())
                .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
                .radius(Corner::Control)
                .background(fill)
                .border(1.0, edge)
                .on_press(move |launcher: &mut Launcher| launcher.toggle_category(toggled.clone()))
                .any()
        }))
        .wrap_items()
        .gap(Space::SNUG)
        .fill_width()
        .any(),
    )
}

pub(crate) fn view(launcher: &Launcher) -> Element {
    let discover = &launcher.discover;
    let kind = discover.kind;
    let target_index = discover.target.as_deref().and_then(|target| {
        launcher
            .instances
            .iter()
            .position(|instance| instance.id == target)
    });
    let header = column! {
        row! {
            column! {
                text(tr!("launcher.discover.title")).title().bold();
                text(tr!("launcher.discover.subtitle", version = target::rikka().minecraft.as_str()))
                    .caption()
                    .muted();
            }
            .gap(Space::HAIR)
            .grow(1.0);
            text(tr!("launcher.discover.install-into")).label().muted();
            match launcher.instances.is_empty() {
                true => button(labelled(MaterialSymbol::ADD, tr!("launcher.library.new")))
                    .secondary()
                    .on_press(Launcher::begin_creating)
                    .any(),
                false => combo(target_index)
                    .options(launcher.instances.iter().map(|instance| Label::from(instance.config.name.clone())))
                    .on_select(Launcher::set_target)
                    .width(Length::px(220.0))
                    .any(),
            };
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        segmented(KINDS.iter().position(|shown| *shown == kind).unwrap_or_default())
            .segments(KINDS.map(kind_label))
            .on_select(|launcher: &mut Launcher, index: usize| {
                if let Some(kind) = KINDS.get(index) {
                    launcher.set_kind(*kind);
                }
            })
            .width(Length::px(KIND_SWITCH));
        row! {
            text_field(&discover.query)
                .placeholder(tr!("launcher.discover.search", kind = kind_label(kind).to_string().as_str()))
                .on_change(Launcher::query_changed)
                .on_submit(|launcher: &mut Launcher| launcher.search(true))
                .grow(1.0);
            combo(SortIndex::ALL.iter().position(|sort| *sort == discover.sort))
                .options(SortIndex::ALL.map(sort_label))
                .on_select(|launcher: &mut Launcher, index: usize| {
                    if let Some(sort) = SortIndex::ALL.get(index) {
                        launcher.set_sort(*sort);
                    }
                })
                .width(Length::px(SORT));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        categories(launcher);
    }
    .gap(Space::NORMAL)
    .padding(Inset::edges(Space::WIDE, Space::WIDE, Space::NORMAL, Space::WIDE))
    .fill_width();

    let installed = launcher.installed_projects();
    let more = discover.more;
    let body = match &discover.results {
        Load::Idle | Load::Loading => container(spinner().extent(Length::px(28.0)))
            .center()
            .fill()
            .any(),
        Load::Failed(error) => status_page(MaterialSymbol::CLOUD_OFF, error.clone())
            .tint(palette().danger)
            .action(
                button(text(tr!("launcher.action.retry")))
                    .secondary()
                    .on_press(|launcher: &mut Launcher| launcher.search(true)),
            )
            .fill()
            .any(),
        Load::Ready(results) if results.hits.is_empty() => status_page(
            MaterialSymbol::SEARCH_OFF,
            tr!("launcher.discover.no-results"),
        )
        .fill()
        .any(),
        Load::Ready(results) => {
            let shown: Rc<[(SearchHit, InstallState)]> = results
                .hits
                .iter()
                .map(|hit| {
                    (
                        hit.clone(),
                        install_state(launcher, &installed, &hit.project_id),
                    )
                })
                .collect();
            let has_more = (shown.len() as u32) < results.total;
            let count = shown.len() + usize::from(has_more);
            lazy_column(count, move |slot| match shown.get(slot.index) {
                Some((hit, state)) => hit_card(hit, kind, *state),
                None => container(
                    button(text(tr!("launcher.discover.more")))
                        .secondary()
                        .loading(more)
                        .on_press(|launcher: &mut Launcher| launcher.search(false)),
                )
                .center()
                .padding(Inset::all(Space::ROOMY))
                .fill_width()
                .any(),
            })
            .estimate(ROW)
            .gap(Space::SNUG)
            .padding(Inset::edges(
                Space::NONE,
                Space::WIDE,
                Space::WIDE,
                Space::WIDE,
            ))
            .fill()
            .any()
        }
    };
    column! {
        header;
        body.grow(1.0).fill_width();
    }
    .fill()
    .any()
}
