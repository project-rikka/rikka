use std::sync::Arc;

use launcher_game::QuickPlay;
use launcher_instance::{GameMode, Server, World, Worlds};
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{column, row};

use crate::format;
use crate::launcher::{Confirm, Launcher};
use crate::load::{Load, loading};
use crate::pictures;
use crate::runs::Activity;
use crate::widgets::{chip, heading, icon_button, labelled};

const ICON: f32 = 48.0;

pub(crate) struct WorldsState {
    pub(crate) listed: Load<(Vec<World>, Vec<Server>)>,
    pub(crate) server_name: TextDocument,
    pub(crate) server_address: TextDocument,
}

impl Default for WorldsState {
    fn default() -> WorldsState {
        WorldsState {
            listed: Load::Idle,
            server_name: TextDocument::new(""),
            server_address: TextDocument::new(""),
        }
    }
}

type Listed = launcher_instance::Result<(Worlds, Vec<Server>)>;

impl Launcher {
    pub(crate) fn load_worlds(&mut self) {
        let (Some(id), Some(services)) = (self.selected_id(), self.services()) else {
            return;
        };
        if self.instance.worlds.listed.ready().is_none() {
            self.instance.worlds.listed = Load::Loading;
        }
        spawn_blocking_keyed(
            "launcher-worlds",
            move || -> Listed {
                let instance = services.instances.get(&id)?;
                Ok((instance.worlds()?, instance.servers()?))
            },
            |launcher: &mut Launcher, listed: Listed| match listed {
                Ok((worlds, servers)) => {
                    launcher.instance.worlds.listed = Load::Ready((worlds.worlds, servers));
                    if let Some(problem) = worlds.problems.first() {
                        launcher.fail(problem.to_string());
                    }
                }
                Err(error) => launcher.instance.worlds.listed = Load::Failed(error.to_string()),
            },
        );
    }

    pub(crate) fn delete_world(&mut self, id: &str, folder: &str) {
        let Some(services) = self.services() else {
            return;
        };
        let id = id.to_owned();
        let folder = folder.to_owned();
        spawn_blocking(
            move || services.instances.get(&id)?.remove_world(&folder),
            |launcher: &mut Launcher, removed: launcher_instance::Result<()>| {
                if let Err(error) = removed {
                    launcher.fail(error.to_string());
                }
                launcher.load_worlds();
            },
        );
    }

    fn add_server(&mut self) {
        let (Some(id), Some(services)) = (self.selected_id(), self.services()) else {
            return;
        };
        let address = self.instance.worlds.server_address.text().trim().to_owned();
        if address.is_empty() {
            return;
        }
        let name = match self.instance.worlds.server_name.text().trim() {
            "" => address.clone(),
            name => name.to_owned(),
        };
        self.instance.worlds.server_name.reset("");
        self.instance.worlds.server_address.reset("");
        spawn_blocking(
            move || services.instances.get(&id)?.add_server(&name, &address),
            |launcher: &mut Launcher, added: launcher_instance::Result<()>| {
                if let Err(error) = added {
                    launcher.fail(error.to_string());
                }
                launcher.load_worlds();
            },
        );
    }

    pub(crate) fn remove_server(&mut self, id: &str, address: &str) {
        let Some(services) = self.services() else {
            return;
        };
        let id = id.to_owned();
        let address = address.to_owned();
        spawn_blocking(
            move || services.instances.get(&id)?.remove_server(&address),
            |launcher: &mut Launcher, removed: launcher_instance::Result<()>| {
                if let Err(error) = removed {
                    launcher.fail(error.to_string());
                }
                launcher.load_worlds();
            },
        );
    }
}

fn mode_label(world: &World) -> Label {
    match (world.hardcore, world.mode) {
        (true, _) => tr!("launcher.worlds.hardcore"),
        (false, GameMode::Survival) => tr!("launcher.worlds.survival"),
        (false, GameMode::Creative) => tr!("launcher.worlds.creative"),
        (false, GameMode::Adventure) => tr!("launcher.worlds.adventure"),
        (false, GameMode::Spectator) => tr!("launcher.worlds.spectator"),
    }
}

fn world_row(world: &World, instance: &str, idle: bool) -> Element {
    let played = (instance.to_owned(), world.folder.clone());
    let opened = world.path.display().to_string();
    let removed = (
        instance.to_owned(),
        world.folder.clone(),
        world.name.clone(),
    );
    let detail = match world.last_played {
        Some(milliseconds) => format::ago(milliseconds / 1000).to_string(),
        None => world.folder.clone(),
    };
    row! {
        pictures::framed(
            world.icon.clone().map(|path| ImageSource::file(Arc::from(path.as_path()))),
            MaterialSymbol::PUBLIC,
            ICON,
            10.0,
        );
        column! {
            row! {
                text(world.name.clone()).label().semibold().no_wrap().min_width(0.0);
                chip(mode_label(world), match world.hardcore {
                    true => palette().danger,
                    false => palette().accent,
                });
                world.version.as_ref().map(|version| chip(version.clone(), palette().text_muted));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
            text(detail).caption().muted().no_wrap();
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .min_width(0.0);
        icon_button(MaterialSymbol::FOLDER_OPEN, tr!("launcher.instance.open-folder"), move |launcher: &mut Launcher| {
            if let Err(error) = rikka_open::open(&opened) {
                launcher.fail(error.to_string());
            }
        });
        icon_button(MaterialSymbol::DELETE, tr!("launcher.action.delete"), move |launcher: &mut Launcher| {
            launcher.ask(Confirm::DeleteWorld {
                instance: removed.0.clone(),
                folder: removed.1.clone(),
                name: removed.2.clone(),
            });
        });
        button(labelled(MaterialSymbol::PLAY_ARROW, tr!("launcher.action.play")))
            .secondary()
            .enabled(idle)
            .on_press(move |launcher: &mut Launcher| {
                launcher.play(played.0.clone(), Some(QuickPlay::World(played.1.clone())));
            });
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::NORMAL, Space::SNUG))
    .fill_width()
    .any()
}

fn server_row(server: &Server, instance: &str, idle: bool) -> Element {
    let played = (instance.to_owned(), server.address.clone());
    let removed = (
        instance.to_owned(),
        server.address.clone(),
        server.name.clone(),
    );
    row! {
        pictures::framed(
            server.icon.clone().map(|bytes| ImageSource::Bytes(bytes.into())),
            MaterialSymbol::DNS,
            ICON,
            10.0,
        );
        column! {
            text(server.name.clone()).label().semibold().no_wrap();
            text(server.address.clone()).caption().muted().no_wrap();
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .min_width(0.0);
        icon_button(MaterialSymbol::DELETE, tr!("launcher.action.remove"), move |launcher: &mut Launcher| {
            launcher.ask(Confirm::RemoveServer {
                instance: removed.0.clone(),
                address: removed.1.clone(),
                name: removed.2.clone(),
            });
        });
        button(labelled(MaterialSymbol::PLAY_ARROW, tr!("launcher.worlds.join")))
            .secondary()
            .enabled(idle)
            .on_press(move |launcher: &mut Launcher| {
                launcher.play(played.0.clone(), Some(QuickPlay::Server(played.1.clone())));
            });
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::NORMAL, Space::SNUG))
    .fill_width()
    .any()
}

pub(super) fn view(launcher: &Launcher, id: &str) -> Element {
    let state = &launcher.instance.worlds;
    let idle = launcher.runs.activity(id) == Activity::Idle;
    let listed = loading(
        &state.listed,
        |(worlds, servers): &(Vec<World>, Vec<Server>)| {
            let worlds: Vec<Element> = match worlds.is_empty() {
                true => vec![text(tr!("launcher.worlds.no-worlds")).label().muted().any()],
                false => worlds
                    .iter()
                    .map(|world| world_row(world, id, idle))
                    .collect(),
            };
            let servers: Vec<Element> = match servers.is_empty() {
                true => vec![
                    text(tr!("launcher.worlds.no-servers"))
                        .label()
                        .muted()
                        .any(),
                ],
                false => servers
                    .iter()
                    .map(|server| server_row(server, id, idle))
                    .collect(),
            };
            column! {
                heading(tr!("launcher.worlds.worlds"));
                card(column_of(worlds).gap(Space::HAIR).fill_width());
                heading(tr!("launcher.worlds.servers"));
                card(column_of(servers).gap(Space::HAIR).fill_width());
            }
            .gap(Space::NORMAL)
            .fill_width()
        },
    );
    let adding = row! {
        text_field(&state.server_name)
            .placeholder(tr!("launcher.worlds.server-name"))
            .on_change(|| {})
            .width(Length::px(200.0));
        text_field(&state.server_address)
            .placeholder(tr!("launcher.worlds.server-address"))
            .on_change(|| {})
            .on_submit(Launcher::add_server)
            .grow(1.0);
        button(labelled(MaterialSymbol::ADD, tr!("launcher.worlds.add-server")))
            .secondary()
            .enabled(!state.server_address.text().trim().is_empty())
            .on_press(Launcher::add_server);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width();
    scrollable(
        column! {
            listed;
            adding;
        }
        .gap(Space::ROOMY)
        .padding(Inset::edges(
            Space::NONE,
            Space::WIDE,
            Space::WIDE,
            Space::WIDE,
        ))
        .fill_width(),
    )
    .fill()
    .any()
}
