use std::collections::HashMap;
use std::rc::Rc;

use launcher_content::{Identified, Planned};
use launcher_instance::{ContentFile, ContentKind, Instance, Place};
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_ui::Element;
use rikka_widgets::{column, row};

use crate::kinds::{kind_label, kind_symbol};
use crate::launcher::{Confirm, Launcher, Page};
use crate::load::{Load, loading};
use crate::pictures;
use crate::services::{scan, update};
use crate::target;
use crate::widgets::{chip, icon_button, labelled};

const ROW: f32 = 64.0;
const ICON: f32 = 40.0;

pub(crate) struct ContentState {
    pub(crate) files: Load<Rc<[Identified]>>,
    pub(crate) filter: Option<ContentKind>,
    pub(crate) search: TextDocument,
    pub(crate) updates: Load<HashMap<String, Planned>>,
    pub(crate) updating: bool,
}

impl Default for ContentState {
    fn default() -> ContentState {
        ContentState {
            files: Load::Idle,
            filter: None,
            search: TextDocument::new(""),
            updates: Load::Idle,
            updating: false,
        }
    }
}

const FILTERS: [Option<ContentKind>; 5] = [
    None,
    Some(ContentKind::Mod),
    Some(ContentKind::ResourcePack),
    Some(ContentKind::ShaderPack),
    Some(ContentKind::DataPack),
];

fn filter_label(filter: Option<ContentKind>) -> Label {
    match filter {
        None => tr!("launcher.content.all"),
        Some(kind) => kind_label(kind),
    }
}

type Loaded = Result<(Instance, Vec<Identified>, Option<String>), String>;

impl Launcher {
    pub(crate) fn load_content(&mut self) {
        let (Some(id), Some(services)) = (self.selected_id(), self.services()) else {
            return;
        };
        if self.instance.content.files.ready().is_none() {
            self.instance.content.files = Load::Loading;
        }
        spawn_keyed(
            "launcher-content",
            async move {
                let (mut instance, files) = scan(services.clone(), id.clone())
                    .await
                    .map_err(|error| error.to_string())?;
                let (identified, problem) = match services
                    .catalog
                    .identify(&mut instance, files.clone())
                    .await
                {
                    Ok(identified) => (identified, None),
                    Err(error) => (
                        files
                            .into_iter()
                            .map(|file| Identified {
                                file,
                                project: None,
                            })
                            .collect(),
                        Some(error.to_string()),
                    ),
                };
                let content = instance.config.content;
                let instance = update(services, id, move |config| config.content = content)
                    .await
                    .map_err(|error| error.to_string())?;
                Ok((instance, identified, problem))
            },
            |launcher: &mut Launcher, loaded: Loaded| match loaded {
                Ok((instance, identified, problem)) => {
                    launcher.replace_instance(instance);
                    launcher.instance.content.files = Load::Ready(identified.into());
                    if let Some(problem) = problem {
                        launcher.fail(tr!("launcher.content.offline", reason = problem.as_str()));
                    }
                }
                Err(error) => launcher.instance.content.files = Load::Failed(error),
            },
        );
    }

    fn set_content_filter(&mut self, filter: Option<ContentKind>) {
        self.instance.content.filter = filter;
    }

    fn loaded_file(&self, key: &str) -> Option<ContentFile> {
        self.instance
            .content
            .files
            .ready()?
            .iter()
            .find(|identified| identified.file.key == key)
            .map(|identified| identified.file.clone())
    }

    fn toggle_content(&mut self, key: String, enabled: bool) {
        let (Some(id), Some(services), Some(file)) =
            (self.selected_id(), self.services(), self.loaded_file(&key))
        else {
            return;
        };
        spawn_blocking(
            move || services.instances.get(&id)?.set_enabled(&file, enabled),
            |launcher: &mut Launcher, toggled: launcher_instance::Result<()>| {
                if let Err(error) = toggled {
                    launcher.fail(error.to_string());
                }
                launcher.load_content();
            },
        );
    }

    pub(crate) fn remove_content(&mut self, id: &str, key: &str) {
        let (Some(services), Some(file)) = (self.services(), self.loaded_file(key)) else {
            return;
        };
        let id = id.to_owned();
        spawn_blocking(
            move || -> launcher_instance::Result<()> {
                std::fs::remove_file(&file.path).map_err(|source| {
                    launcher_instance::Error::Io {
                        path: file.path.clone(),
                        source,
                    }
                })?;
                services.instances.update(&id, |config| {
                    config.content.remove(&file.key);
                })?;
                Ok(())
            },
            |launcher: &mut Launcher, removed: launcher_instance::Result<()>| {
                if let Err(error) = removed {
                    launcher.fail(error.to_string());
                }
                launcher.load_content();
            },
        );
    }

    fn check_updates(&mut self) {
        let (Some(services), Some(files)) = (self.services(), self.instance.content.files.ready())
        else {
            return;
        };
        let files: Vec<_> = files
            .iter()
            .map(|identified| identified.file.clone())
            .collect();
        self.instance.content.updates = Load::Loading;
        let game = target::rikka().minecraft;
        spawn_keyed(
            "launcher-updates",
            async move { services.catalog.updates(&files, &game).await },
            |launcher: &mut Launcher, found: launcher_content::Result<Vec<Planned>>| match found {
                Ok(found) => {
                    let count = found.len();
                    launcher.instance.content.updates = Load::Ready(
                        found
                            .into_iter()
                            .filter_map(|planned| {
                                Some((planned.replaces.as_ref()?.key.clone(), planned))
                            })
                            .collect(),
                    );
                    if count == 0 {
                        launcher.tell(tr!("launcher.content.up-to-date"));
                    }
                }
                Err(error) => {
                    launcher.instance.content.updates = Load::Idle;
                    launcher.fail(error.to_string());
                }
            },
        );
    }

    fn apply_updates(&mut self, keys: Vec<String>) {
        let (Some(id), Some(services), Some(updates)) = (
            self.selected_id(),
            self.services(),
            self.instance.content.updates.ready(),
        ) else {
            return;
        };
        let items: Vec<Planned> = keys
            .iter()
            .filter_map(|key| updates.get(key).cloned())
            .collect();
        if items.is_empty() {
            return;
        }
        self.instance.content.updating = true;
        spawn_keyed(
            "launcher-apply-updates",
            async move {
                let mut instance = services
                    .instances
                    .get(&id)
                    .map_err(|error| error.to_string())?;
                let plan = launcher_content::Plan { items };
                services
                    .catalog
                    .apply(&mut instance, &plan)
                    .await
                    .map_err(|error| error.to_string())?;
                let content = instance.config.content;
                update(services, id, move |config| config.content = content)
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(keys)
            },
            |launcher: &mut Launcher, applied: Result<Vec<String>, String>| {
                launcher.instance.content.updating = false;
                match applied {
                    Ok(keys) => {
                        if let Load::Ready(updates) = &mut launcher.instance.content.updates {
                            for key in &keys {
                                updates.remove(key);
                            }
                        }
                        launcher.tell(tr!(
                            "launcher.content.updated",
                            count = &keys.len().to_string()
                        ));
                    }
                    Err(error) => launcher.fail(error),
                }
                launcher.load_content();
            },
        );
    }

    fn import_content(&mut self) {
        let Some(id) = self.selected_id() else {
            return;
        };
        let Some(services) = self.services() else {
            return;
        };
        let filter = self.instance.content.filter;
        let chooser = Chooser::open(tr!("launcher.content.import-title").to_string())
            .parent(window_identifier())
            .multiple(true)
            .filter(
                Filter::named(tr!("launcher.content.import-filter").to_string())
                    .pattern("*.jar")
                    .pattern("*.zip"),
            );
        spawn_blocking(
            move || -> Result<usize, String> {
                let picked = match chooser.run().map_err(|error| error.to_string())? {
                    ChooserResponse::Files(files) => files,
                    ChooserResponse::Cancelled => return Ok(0),
                };
                let mut instance = services
                    .instances
                    .get(&id)
                    .map_err(|error| error.to_string())?;
                let mut imported = 0;
                for path in picked {
                    let kind = import_kind(&path, filter);
                    instance
                        .import(kind, &Place::Instance, &path)
                        .map_err(|error| error.to_string())?;
                    imported += 1;
                }
                let content = instance.config.content;
                services
                    .instances
                    .update(&id, move |config| config.content = content)
                    .map_err(|error| error.to_string())?;
                Ok(imported)
            },
            |launcher: &mut Launcher, imported: Result<usize, String>| match imported {
                Ok(0) => {}
                Ok(count) => {
                    launcher.tell(tr!("launcher.content.imported", count = &count.to_string()));
                    launcher.load_content();
                }
                Err(error) => launcher.fail(error),
            },
        );
    }

    fn browse_content(&mut self) {
        let kind = self.instance.content.filter.unwrap_or(ContentKind::Mod);
        let selected = self.selected_id();
        self.discover.browse_for(selected, kind);
        self.go(Page::Discover);
    }
}

fn import_kind(path: &std::path::Path, filter: Option<ContentKind>) -> ContentKind {
    let jar = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"));
    match (jar, filter) {
        (true, _) => ContentKind::Mod,
        (false, Some(ContentKind::ShaderPack)) => ContentKind::ShaderPack,
        (false, _) => ContentKind::ResourcePack,
    }
}

#[derive(Clone)]
struct Line {
    key: String,
    title: String,
    detail: String,
    kind: ContentKind,
    enabled: bool,
    incompatible: bool,
    icon_url: Option<String>,
    local_icon: Option<std::sync::Arc<[u8]>>,
    project: Option<String>,
    update: Option<String>,
    world: Option<String>,
}

fn line(identified: &Identified, updates: Option<&HashMap<String, Planned>>) -> Line {
    let file = &identified.file;
    let project = identified.project.as_ref();
    let title = project.map_or_else(|| file.title().to_owned(), |project| project.title.clone());
    let version = file.local.as_ref().and_then(|local| local.version.clone());
    let detail = match version {
        Some(version) => format!("{version} · {}", file.file_name),
        None => file.file_name.clone(),
    };
    Line {
        key: file.key.clone(),
        title,
        detail,
        kind: file.kind,
        enabled: file.enabled,
        incompatible: file.record.incompatible,
        icon_url: project.and_then(|project| project.icon_url.clone()),
        local_icon: file.local.as_ref().and_then(|local| local.icon.clone()),
        project: project.map(|project| project.id.clone()),
        update: updates
            .and_then(|updates| updates.get(&file.key))
            .map(|planned| planned.version.version_number.clone()),
        world: match &file.place {
            Place::World(world) => Some(world.clone()),
            Place::Instance => None,
        },
    }
}

fn row_view(line: &Line, instance: &str) -> Element {
    let picture = match (&line.icon_url, &line.local_icon) {
        (Some(url), _) => pictures::remote(Some(url), kind_symbol(line.kind), ICON, 10.0),
        (None, Some(bytes)) => pictures::framed(
            Some(ImageSource::Bytes(bytes.clone())),
            kind_symbol(line.kind),
            ICON,
            10.0,
        ),
        (None, None) => pictures::placeholder(kind_symbol(line.kind), ICON, 10.0),
    };
    let toggled = line.key.clone();
    let removed = (instance.to_owned(), line.key.clone(), line.title.clone());
    let updated = line.key.clone();
    let opened = line.project.clone();
    row! {
        picture;
        column! {
            row! {
                text(line.title.clone()).label().semibold().no_wrap().min_width(0.0);
                if line.incompatible {
                    chip(tr!("launcher.content.incompatible"), palette().danger);
                }
                line.world.as_ref().map(|world| chip(world.clone(), palette().text_muted));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
            text(line.detail.clone()).caption().muted().no_wrap();
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .min_width(0.0);
        line.update.as_ref().map(|version| {
            button(labelled(MaterialSymbol::DOWNLOAD, tr!("launcher.content.update-to", version = version.as_str())))
                .secondary()
                .small()
                .on_press(move |launcher: &mut Launcher| launcher.apply_updates(vec![updated.clone()]))
        });
        opened.map(|project| {
            icon_button(MaterialSymbol::OPEN_IN_NEW, tr!("launcher.content.open-project"), move |launcher: &mut Launcher| {
                launcher.open_project(project.clone());
            })
        });
        switch(line.enabled).on_toggle(move |launcher: &mut Launcher, on: bool| {
            launcher.toggle_content(toggled.clone(), on);
        });
        icon_button(MaterialSymbol::DELETE, tr!("launcher.action.remove"), move |launcher: &mut Launcher| {
            launcher.ask(Confirm::RemoveContent {
                instance: removed.0.clone(),
                key: removed.1.clone(),
                title: removed.2.clone(),
            });
        });
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::NORMAL, Space::SNUG))
    .height(ROW)
    .fill_width()
    .any()
}

pub(super) fn view(launcher: &Launcher, id: &str) -> Element {
    let state = &launcher.instance.content;
    let updates = state.updates.ready();
    let update_keys: Vec<String> = updates
        .map(|updates| updates.keys().cloned().collect())
        .unwrap_or_default();
    let toolbar = row! {
        segmented(FILTERS.iter().position(|filter| *filter == state.filter).unwrap_or_default())
            .segments(FILTERS.map(filter_label))
            .on_select(|launcher: &mut Launcher, index: usize| {
                if let Some(filter) = FILTERS.get(index) {
                    launcher.set_content_filter(*filter);
                }
            });
        text_field(&state.search)
            .placeholder(tr!("launcher.content.search"))
            .on_change(|| {})
            .width(Length::px(220.0));
        spacer().grow(1.0);
        match update_keys.is_empty() {
            true => button(labelled(MaterialSymbol::UPDATE, tr!("launcher.content.check-updates")))
                .ghost()
                .loading(matches!(state.updates, Load::Loading))
                .on_press(Launcher::check_updates)
                .any(),
            false => button(labelled(
                MaterialSymbol::DOWNLOAD,
                tr!("launcher.content.update-all", count = &update_keys.len().to_string()),
            ))
            .secondary()
            .loading(state.updating)
            .on_press(move |launcher: &mut Launcher| launcher.apply_updates(update_keys.clone()))
            .any(),
        };
        button(labelled(MaterialSymbol::UPLOAD_FILE, tr!("launcher.content.import")))
            .ghost()
            .on_press(Launcher::import_content);
        button(labelled(MaterialSymbol::TRAVEL_EXPLORE, tr!("launcher.content.browse")))
            .primary()
            .on_press(Launcher::browse_content);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::WIDE, Space::NONE))
    .fill_width();
    let search = state.search.text().to_lowercase();
    let filter = state.filter;
    let instance = id.to_owned();
    let list = loading(&state.files, move |files: &Rc<[Identified]>| {
        let lines: Rc<[Line]> = files
            .iter()
            .filter(|identified| filter.is_none_or(|kind| identified.file.kind == kind))
            .map(|identified| line(identified, updates))
            .filter(|line| search.is_empty() || line.title.to_lowercase().contains(&search))
            .collect();
        if lines.is_empty() {
            return status_page(MaterialSymbol::EXTENSION, tr!("launcher.content.empty"))
                .fill()
                .any();
        }
        let count = lines.len();
        let instance = instance.clone();
        lazy_column(count, move |slot| row_view(&lines[slot.index], &instance))
            .estimate(ROW)
            .padding(Inset::edges(
                Space::NONE,
                Space::LOOSE,
                Space::WIDE,
                Space::LOOSE,
            ))
            .fill()
            .any()
    });
    column! {
        toolbar;
        list.grow(1.0).fill_width();
    }
    .gap(Space::NORMAL)
    .fill()
    .any()
}
