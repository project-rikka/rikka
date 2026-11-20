use rikka_app::prelude::*;
use rikka_nix::{Generation, Module};
use rikka_widgets::column;

use crate::host_name;
use crate::page::Page;
use crate::pending::Pending;
use crate::session::Session;
use crate::system::generations::{self, Generations};
use crate::system::nix::{self, Nix};

const HOST_NAME: &str = "networking.hostName";

const NAME_FIELD: f32 = 220.0;

const NAME_LIMIT: usize = 63;

pub(crate) struct System {
    hostname: TextDocument,
    nix: Nix,
    generations: Generations,
}

impl System {
    pub(crate) fn new() -> Self {
        Self {
            hostname: TextDocument::new(""),
            nix: Nix::new(),
            generations: Generations::new(),
        }
    }

    pub(crate) fn rebuild_finished(&mut self) {
        reread_generations();
    }
}

fn reread_generations() {
    spawn_blocking(
        generations::listed,
        |system: &mut System, listed: Result<Vec<Generation>, String>| {
            system.generations.read(listed);
        },
    );
}

impl Page for System {
    fn opened(&mut self, pending: &Pending, _: &Session) {
        self.hostname = TextDocument::new(
            pending
                .get::<String>(Module::System, HOST_NAME)
                .unwrap_or_default(),
        );
        self.nix.opened();
        reread_generations();
    }
}

pub(crate) fn view(system: &System, pending: &Pending) -> impl View {
    column! {
        machine(system);
        nix::view(&system.nix, pending);
        generations::view(&system.generations, pending);
    }
    .gap(Space::ROOMY)
}

fn machine(system: &System) -> impl View {
    let typed = system.hostname.text();
    let hostname = system.hostname.clone();
    section(
        tr!("settings.system.machine"),
        field(
            tr!("settings.system.host-name"),
            text_field(&system.hostname)
                .max_len(NAME_LIMIT)
                .error(!typed.is_empty() && !host_name::is_label(&typed))
                .on_change(move |pending: &mut Pending| {
                    write_host_name(pending, &hostname.text());
                })
                .width(NAME_FIELD),
        ),
    )
}

fn write_host_name(pending: &mut Pending, name: &str) {
    if name.is_empty() {
        pending.remove(Module::System, HOST_NAME);
    } else if host_name::is_label(name) {
        pending.set(Module::System, HOST_NAME, name);
    }
}

impl std::fmt::Debug for System {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("System").finish_non_exhaustive()
    }
}
