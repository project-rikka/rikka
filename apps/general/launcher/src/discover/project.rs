use std::rc::Rc;

use launcher_content::{content_kind, loaders};
use launcher_instance::ContentKind;
use launcher_modrinth::{Project, TeamMember, Version, VersionFilter, VersionType};
use rikka_app::prelude::*;
use rikka_markdown::markdown;
use rikka_ui::Element;
use rikka_widgets::{column, row};

use super::{InstallState, install_button, install_state};
use crate::format;
use crate::kinds::{kind_label, kind_symbol};
use crate::launcher::{Launcher, Page};
use crate::load::{Load, loading};
use crate::pictures;
use crate::target;
use crate::widgets::{chip, icon_button, labelled, stat};

const ICON: f32 = 96.0;
const GALLERY: f32 = 300.0;
const GALLERY_HEIGHT: f32 = 170.0;
const BODY: f32 = 860.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum Section {
    #[default]
    Description,
    Versions,
    Gallery,
}

impl Section {
    const ALL: [Section; 3] = [Section::Description, Section::Versions, Section::Gallery];

    fn label(self) -> Label {
        match self {
            Section::Description => tr!("launcher.project.description"),
            Section::Versions => tr!("launcher.project.versions"),
            Section::Gallery => tr!("launcher.project.gallery"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProjectPage {
    id: String,
    project: Load<Project>,
    versions: Load<Rc<[Version]>>,
    members: Load<Vec<TeamMember>>,
    section: Section,
}

impl Launcher {
    pub(crate) fn open_project(&mut self, id: String) {
        let Some(services) = self.services() else {
            return;
        };
        self.project = Some(ProjectPage {
            id: id.clone(),
            project: Load::Loading,
            versions: Load::Loading,
            members: Load::Loading,
            section: Section::Description,
        });
        self.go(Page::Project);
        let fetched = services.clone();
        let wanted = id.clone();
        spawn_keyed(
            "launcher-project",
            async move { fetched.catalog.modrinth().project(&wanted).await },
            |launcher: &mut Launcher, found: launcher_modrinth::Result<Project>| {
                let Some(page) = &mut launcher.project else {
                    return;
                };
                match found {
                    Ok(project) => {
                        let kind = content_kind(project.project_type);
                        let id = project.id.clone();
                        page.project = Load::Ready(project);
                        launcher.load_versions(id, kind);
                    }
                    Err(error) => {
                        page.project = Load::Failed(error.to_string());
                        page.versions = Load::Idle;
                    }
                }
            },
        );
        spawn_keyed(
            "launcher-project-members",
            async move { services.catalog.modrinth().members(&id).await },
            |launcher: &mut Launcher, found: launcher_modrinth::Result<Vec<TeamMember>>| {
                if let Some(page) = &mut launcher.project {
                    page.members = match found {
                        Ok(mut members) => {
                            members.sort_by_key(|member| member.ordering);
                            Load::Ready(members)
                        }
                        Err(error) => Load::Failed(error.to_string()),
                    };
                }
            },
        );
    }

    fn load_versions(&mut self, id: String, kind: Option<ContentKind>) {
        let (Some(services), Some(kind)) = (self.services(), kind) else {
            if let Some(page) = &mut self.project {
                page.versions = Load::Ready(Rc::from([]));
            }
            return;
        };
        let filter = VersionFilter {
            loaders: loaders(kind),
            game_versions: vec![target::rikka().minecraft],
        };
        spawn_keyed(
            "launcher-project-versions",
            async move {
                services
                    .catalog
                    .modrinth()
                    .project_versions(&id, &filter)
                    .await
            },
            |launcher: &mut Launcher, found: launcher_modrinth::Result<Vec<Version>>| {
                if let Some(page) = &mut launcher.project {
                    page.versions = match found {
                        Ok(versions) => Load::Ready(versions.into()),
                        Err(error) => Load::Failed(error.to_string()),
                    };
                }
            },
        );
    }

    fn set_section(&mut self, section: Section) {
        if let Some(page) = &mut self.project {
            page.section = section;
        }
    }

    fn open_url(&mut self, url: String) {
        if let Err(error) = rikka_open::open(&url) {
            self.fail(error.to_string());
        }
    }
}

fn version_type(kind: VersionType) -> Element {
    match kind {
        VersionType::Release => chip(tr!("launcher.project.release"), palette().success),
        VersionType::Beta => chip(tr!("launcher.project.beta"), palette().warning),
        VersionType::Alpha => chip(tr!("launcher.project.alpha"), palette().danger),
    }
}

fn unavailable() -> Element {
    button(text(tr!("launcher.project.incompatible")))
        .secondary()
        .enabled(false)
        .any()
}

fn link(symbol: MaterialSymbol, label: Label, url: Option<&String>) -> Option<Element> {
    let url = url.filter(|url| !url.is_empty())?.clone();
    Some(icon_button(
        symbol,
        label,
        move |launcher: &mut Launcher| {
            launcher.open_url(url.clone());
        },
    ))
}

fn header(launcher: &Launcher, project: &Project, page: &ProjectPage) -> Element {
    let installed = launcher.installed_projects();
    let state = install_state(launcher, &installed, &project.id);
    let authors = page
        .members
        .ready()
        .map(|members| {
            members
                .iter()
                .map(|member| member.user.username.clone())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let kind = content_kind(project.project_type);
    let compatible = page
        .versions
        .ready()
        .is_some_and(|versions| !versions.is_empty());
    let shown = match (state, page.versions.ready(), kind) {
        (InstallState::Available, Some(_), Some(_)) if !compatible => unavailable(),
        (_, _, None) => unavailable(),
        (_, _, Some(kind)) => install_button(state, project.id.clone(), kind, None),
    };
    row! {
        pictures::remote(
            project.icon_url.as_deref(),
            kind.map_or(MaterialSymbol::EXTENSION, kind_symbol),
            ICON,
            20.0,
        );
        column! {
            text(project.title.clone()).title().bold().fill_width();
            text(project.description.clone()).label().muted().fill_width();
            row! {
                kind.map(|kind| chip(kind_label(kind), palette().accent));
                stat(MaterialSymbol::DOWNLOAD, format::count(project.downloads));
                stat(MaterialSymbol::FAVORITE, format::count(project.followers));
                format::iso_ago(&project.updated).map(|updated| stat(MaterialSymbol::UPDATE, updated));
                (!authors.is_empty()).then(|| stat(MaterialSymbol::GROUP, authors.clone()));
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center);
        }
        .gap(Space::SNUG)
        .grow(1.0)
        .min_width(0.0);
        column! {
            shown;
            row! {
                link(MaterialSymbol::LANGUAGE, tr!("launcher.project.modrinth"), Some(&project.page_url()));
                link(MaterialSymbol::CODE, tr!("launcher.project.source"), project.source_url.as_ref());
                link(MaterialSymbol::BUG_REPORT, tr!("launcher.project.issues"), project.issues_url.as_ref());
                link(MaterialSymbol::MENU_BOOK, tr!("launcher.project.wiki"), project.wiki_url.as_ref());
                link(MaterialSymbol::FORUM, tr!("launcher.project.discord"), project.discord_url.as_ref());
            }
            .gap(Space::HAIR);
        }
        .gap(Space::SNUG)
        .align_items(Align::End);
    }
    .gap(Space::LOOSE)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn version_row(launcher: &Launcher, project: &Project, version: &Version) -> Element {
    let installed = launcher.installed_projects();
    let state = install_state(launcher, &installed, &project.id);
    let date = format::iso_ago(&version.date_published)
        .map(|date| date.to_string())
        .unwrap_or_default();
    row! {
        column! {
            row! {
                text(version.name.clone()).label().semibold().no_wrap().min_width(0.0);
                version_type(version.version_type);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
            text(format!("{} · {} · {}", version.version_number, version.loaders.join(", "), date))
                .caption()
                .muted()
                .no_wrap();
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .min_width(0.0);
        stat(MaterialSymbol::DOWNLOAD, format::count(version.downloads));
        match (state, content_kind(project.project_type)) {
            (InstallState::Available | InstallState::NoTarget, Some(kind)) => install_button(
                state,
                project.id.clone(),
                kind,
                Some(version.id.clone()),
            ),
            _ => Element::empty(),
        };
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::NORMAL, Space::SNUG))
    .fill_width()
    .any()
}

fn section(launcher: &Launcher, project: &Project, page: &ProjectPage) -> Element {
    match page.section {
        Section::Description => markdown(project.body.clone())
            .on_link(|launcher: &mut Launcher, url: String| launcher.open_url(url))
            .max_width(BODY)
            .fill_width()
            .any(),
        Section::Versions => loading(&page.versions, |versions: &Rc<[Version]>| {
            match versions.is_empty() {
                true => text(tr!("launcher.project.no-versions", version = target::rikka().minecraft.as_str()))
                    .label()
                    .muted()
                    .any(),
                false => card(
                    column_of(versions.iter().map(|version| version_row(launcher, project, version)))
                        .gap(Space::HAIR)
                        .fill_width(),
                )
                .any(),
            }
        }),
        Section::Gallery => match project.gallery.is_empty() {
            true => text(tr!("launcher.project.no-gallery")).label().muted().any(),
            false => column_of(project.gallery.iter().map(|picture| {
                let url = picture.url.clone();
                column! {
                    container(
                        pictures::fetched(&url)
                            .ready()
                            .cloned()
                            .map(|bytes| image(ImageSource::Bytes(bytes)).fit(ImageFit::Contain).fill().any())
                            .unwrap_or_else(|| spinner().extent(Length::px(20.0)).any()),
                    )
                    .center()
                    .background(palette().surface_variant)
                    .radius(Corner::Panel)
                    .clip_content()
                    .height(Length::px(GALLERY_HEIGHT * 2.0))
                    .fill_width();
                    picture.title.as_ref().map(|title| text(title.clone()).label().semibold());
                    picture.description.as_ref().map(|description| text(description.clone()).caption().muted());
                }
                .gap(Space::TIGHT)
                .max_width(GALLERY * 3.0)
                .fill_width()
                .any()
            }))
            .gap(Space::LOOSE)
            .fill_width()
            .any(),
        },
    }
}

pub(crate) fn project_view(launcher: &Launcher) -> Element {
    let Some(page) = &launcher.project else {
        return Element::empty();
    };
    let back = row! {
        button(labelled(MaterialSymbol::ARROW_BACK, tr!("launcher.action.back")))
            .ghost()
            .on_press(Launcher::back);
    };
    let content = loading(&page.project, |project: &Project| {
        column! {
            header(launcher, project, page);
            row! {
                segmented(Section::ALL.iter().position(|shown| *shown == page.section).unwrap_or_default())
                    .segments(Section::ALL.map(Section::label))
                    .on_select(|launcher: &mut Launcher, index: usize| {
                        if let Some(section) = Section::ALL.get(index) {
                            launcher.set_section(*section);
                        }
                    });
                spacer().grow(1.0);
                project.license.as_ref().map(|license| chip(license.id.clone(), palette().text_muted));
            }
            .align_items(Align::Center)
            .fill_width();
            section(launcher, project, page);
        }
        .gap(Space::ROOMY)
        .fill_width()
    });
    scrollable(
        column! {
            back;
            content;
        }
        .gap(Space::NORMAL)
        .padding(Inset::all(Space::WIDE))
        .fill_width(),
    )
    .fill()
    .build()
    .keyed(("launcher-project-page", page.id.clone()))
}
