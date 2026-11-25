use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use launcher_instance::{Instance, Listing};
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{column, row};

use crate::format;
use crate::launcher::{Confirm, Launcher, Page, STAGE};
use crate::pictures;
use crate::runs::Activity;
use crate::target;
use crate::widgets::{chip, lift, stat};

const CARD: f32 = 252.0;
const ICON: f32 = 56.0;
const SHEET: f32 = 420.0;

pub(crate) struct Creating {
    pub(crate) name: TextDocument,
}

#[derive(Clone, Debug, PartialEq)]
struct Tile {
    id: String,
    name: String,
    icon: Option<PathBuf>,
    play_time: u64,
    last_played: Option<i64>,
    activity: Activity,
}

impl Launcher {
    pub(crate) fn reload_instances(&mut self) {
        let Some(services) = self.services() else {
            return;
        };
        spawn_blocking(
            move || services.instances.list(),
            |launcher: &mut Launcher, listed: launcher_instance::Result<Listing>| match listed {
                Ok(listing) => {
                    launcher.instances = listing.instances;
                    if let Some(problem) = listing.problems.first() {
                        launcher.fail(problem.to_string());
                    }
                    if launcher
                        .selected
                        .as_deref()
                        .is_some_and(|id| launcher.instance(id).is_none())
                    {
                        launcher.selected = None;
                    }
                }
                Err(error) => launcher.fail(error.to_string()),
            },
        );
    }

    pub(crate) fn begin_creating(&mut self) {
        self.creating = Some(Creating {
            name: TextDocument::new(""),
        });
    }

    fn cancel_creating(&mut self) {
        self.creating = None;
    }

    fn create_instance(&mut self) {
        let Some(creating) = &self.creating else {
            return;
        };
        let name = creating.name.text().trim().to_owned();
        if name.is_empty() {
            return;
        }
        let Some(services) = self.services() else {
            return;
        };
        self.creating = None;
        spawn_blocking(
            move || services.instances.create(&name),
            |launcher: &mut Launcher, created: launcher_instance::Result<Instance>| match created {
                Ok(instance) => {
                    let id = instance.id.clone();
                    launcher.instances.insert(0, instance);
                    launcher.open_instance(id);
                }
                Err(error) => launcher.fail(error.to_string()),
            },
        );
    }

    pub(crate) fn open_instance(&mut self, id: String) {
        self.selected = Some(id);
        self.instance = crate::instance::InstancePage::default();
        self.go(Page::Instance);
        self.load_instance_tab();
    }

    pub(crate) fn delete_instance(&mut self, id: &str) {
        if self.runs.activity(id) != Activity::Idle {
            self.fail(tr!("launcher.library.delete-running"));
            return;
        }
        let Some(services) = self.services() else {
            return;
        };
        let id = id.to_owned();
        spawn_blocking(
            move || services.instances.remove(&id).map(|()| id),
            |launcher: &mut Launcher, removed: launcher_instance::Result<String>| match removed {
                Ok(id) => {
                    launcher.instances.retain(|instance| instance.id != id);
                    if launcher.selected.as_deref() == Some(id.as_str()) {
                        launcher.selected = None;
                        launcher.go(Page::Library);
                    }
                }
                Err(error) => launcher.fail(error.to_string()),
            },
        );
    }

    pub(crate) fn ask_delete_instance(&mut self, id: String) {
        self.ask(Confirm::DeleteInstance(id));
    }
}

pub(crate) fn view(launcher: &Launcher) -> impl View {
    let target = target::rikka();
    let header = row! {
        column! {
            text(tr!("launcher.library.title")).title().bold();
            text(tr!(
                "launcher.library.subtitle",
                minecraft = target.minecraft.as_str(),
                loader = target.fabric_loader.as_str()
            ))
            .caption()
            .muted();
        }
        .gap(Space::HAIR)
        .grow(1.0);
        button(crate::widgets::labelled(MaterialSymbol::ADD, tr!("launcher.library.new")))
            .primary()
            .on_press(Launcher::begin_creating);
    }
    .align_items(Align::Center)
    .padding(Inset::edges(
        Space::WIDE,
        Space::WIDE,
        Space::ROOMY,
        Space::WIDE,
    ))
    .fill_width();
    let tiles: Rc<[Tile]> = launcher
        .instances
        .iter()
        .map(|instance| Tile {
            id: instance.id.clone(),
            name: instance.config.name.clone(),
            icon: instance.icon().map(Path::to_path_buf),
            play_time: instance.config.play_time,
            last_played: instance.config.last_played,
            activity: launcher.runs.activity(&instance.id),
        })
        .collect();
    let body = match tiles.is_empty() {
        true => status_page(
            MaterialSymbol::SPORTS_ESPORTS,
            tr!("launcher.library.empty"),
        )
        .action(
            button(text(tr!("launcher.library.new")))
                .primary()
                .on_press(Launcher::begin_creating),
        )
        .grow(1.0)
        .fill_width()
        .any(),
        false => {
            let count = tiles.len();
            lazy_tiles(Length::px(CARD), count, move |slot| {
                tile(&tiles[slot.index])
            })
            .gap(Space::ROOMY)
            .padding(Inset::edges(
                Space::NONE,
                Space::WIDE,
                Space::WIDE,
                Space::WIDE,
            ))
            .grow(1.0)
            .fill_width()
            .any()
        }
    };
    column! {
        header;
        body;
    }
    .fill()
}

fn tile(tile: &Tile) -> Element {
    let opened = tile.id.clone();
    let played = tile.id.clone();
    let stopped = tile.id.clone();
    let action = match &tile.activity {
        Activity::Idle => button(crate::widgets::labelled(
            MaterialSymbol::PLAY_ARROW,
            tr!("launcher.action.play"),
        ))
        .primary()
        .on_press(move |launcher: &mut Launcher| launcher.play(played.clone(), None))
        .fill_width()
        .any(),
        Activity::Preparing(shown) => column! {
            match shown {
                Some(fraction) => progress(*fraction).fill_width().any(),
                None => progress_indeterminate().fill_width().any(),
            };
            text(tr!("launcher.run.preparing")).caption().muted();
        }
        .gap(Space::TIGHT)
        .fill_width()
        .any(),
        Activity::Running => button(crate::widgets::labelled(
            MaterialSymbol::STOP,
            tr!("launcher.action.stop"),
        ))
        .danger()
        .on_press(move |launcher: &mut Launcher| launcher.stop(&stopped))
        .fill_width()
        .any(),
    };
    let played_at = match tile.last_played {
        Some(unix) => format::ago(unix),
        None => tr!("launcher.time.never-played"),
    };
    lift(
        container(
            column! {
                row! {
                    pictures::framed(
                        tile.icon.clone().map(|path| ImageSource::file(Arc::from(path.as_path()))),
                        MaterialSymbol::SPORTS_ESPORTS,
                        ICON,
                        14.0,
                    );
                    column! {
                        text(tile.name.clone()).label().semibold().no_wrap().fill_width();
                        if tile.activity == Activity::Running {
                            chip(tr!("launcher.run.playing"), palette().success);
                        }
                    }
                    .gap(Space::TIGHT)
                    .grow(1.0)
                    .min_width(0.0);
                }
                .gap(Space::NORMAL)
                .align_items(Align::Center)
                .fill_width();
                row! {
                    stat(MaterialSymbol::SCHEDULE, format::play_time(tile.play_time));
                    stat(MaterialSymbol::HISTORY, played_at);
                }
                .gap(Space::NORMAL)
                .fill_width();
                action;
            }
            .gap(Space::NORMAL)
            .fill_width(),
        )
        .padding(Inset::all(Space::ROOMY))
        .surface()
        .radius(Corner::Panel)
        .border(1.0, palette().outline)
        .on_press(move |launcher: &mut Launcher| launcher.open_instance(opened.clone()))
        .width(Length::px(CARD)),
    )
}

pub(crate) fn creating_sheet(launcher: &Launcher) -> Vec<Element> {
    let Some(creating) = &launcher.creating else {
        return Vec::new();
    };
    sheet(
        column! {
            text(tr!("launcher.library.new")).label().semibold();
            text(tr!("launcher.library.new-body")).caption().muted().fill_width();
            text_field(&creating.name)
                .placeholder(tr!("launcher.library.name"))
                .autofocus(true)
                .on_change(|| {})
                .on_submit(Launcher::create_instance)
                .fill_width();
            row! {
                spacer().grow(1.0);
                button(text(tr!("launcher.action.cancel")))
                    .ghost()
                    .on_press(Launcher::cancel_creating);
                button(text(tr!("launcher.action.create")))
                    .primary()
                    .enabled(!creating.name.text().trim().is_empty())
                    .on_press(Launcher::create_instance);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
        }
        .gap(Space::ROOMY)
        .padding(20.0)
        .fill_width(),
    )
    .width(SHEET)
    .on_dismiss(Launcher::cancel_creating)
    .keyed("launcher-creating")
    .centered_on(STAGE)
    .with_scrim()
}
