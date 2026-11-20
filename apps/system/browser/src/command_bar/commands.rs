use rikka_app::prelude::*;
use rikka_omnibox::{Candidate, Candidates};

use crate::command::Command;
use crate::menus::space_name;
use crate::model::Model;
use crate::omnibox::target::{Act, Target};
use crate::shell::Shell;
use browser_model::Home;

struct Entry {
    act: Act,
    name: String,
    aliases: &'static str,
}

pub(crate) fn commands(shell: &Shell, model: &Model, candidates: &mut Candidates<Target>) {
    for entry in entries(shell, model) {
        let candidate = Candidate::command(Target::Act(entry.act), &entry.name)
            .keywords(entry.aliases.split(' ').filter(|alias| !alias.is_empty()));
        if let Err(error) = candidates.push(candidate) {
            tracing::debug!(%error, "skipping command");
        }
    }
}

fn entries(shell: &Shell, model: &Model) -> Vec<Entry> {
    let window = shell.id();
    let private = shell.private();
    let active = model.active(window);
    let web = active
        .is_some_and(|tab_entry| tab_entry.internal.is_none() && !tab_entry.meta.url.is_empty());
    let pinned = active.is_some_and(|tab_entry| matches!(tab_entry.home, Home::Pinned(_)));
    let readable = active.is_some_and(|active_tab| {
        shell.reader.offers(active_tab.id) || shell.reader.reads(active_tab.id)
    });
    let translated = active.is_some_and(|active_tab| {
        shell
            .browser()
            .translations()
            .tabs()
            .shows_translation(active_tab.id)
    });
    let here = model.space_of(window).map(|space| space.id);
    let mut entries = vec![
        run(Command::NewTab, tr!("browser.command.new-tab"), "tab open"),
        run(
            Command::NewWindow,
            tr!("browser.command.new-window"),
            "window",
        ),
        run(
            Command::PrivateWindow,
            tr!("browser.command.private-window"),
            "incognito private",
        ),
        run(
            Command::ReopenTab,
            tr!("browser.command.reopen-tab"),
            "undo closed restore",
        ),
        run(
            Command::Settings,
            tr!("browser.command.settings"),
            "preferences options",
        ),
        run(Command::History, tr!("browser.command.history"), "visited"),
        Entry {
            act: Act::Archive,
            name: tr!("browser.command.archive").to_string(),
            aliases: "archived closed",
        },
        run(
            Command::ToggleSplit,
            tr!("browser.command.split"),
            "split view side",
        ),
        run(
            Command::Downloads,
            tr!("browser.command.downloads"),
            "downloaded files",
        ),
        run(
            Command::ToggleSidebar,
            tr!("browser.command.sidebar"),
            "sidebar pin",
        ),
        run(
            Command::CloseWindow,
            tr!("browser.command.close-window"),
            "quit",
        ),
    ];
    if !private {
        entries.push(Entry {
            act: Act::NewSpace,
            name: tr!("browser.command.new-space").to_string(),
            aliases: "space workspace",
        });
        entries.push(run(
            Command::ClearData,
            tr!("browser.command.clear-data"),
            "delete cookies cache",
        ));
        for space in model
            .spaces
            .list()
            .iter()
            .filter(|space| Some(space.id) != here)
        {
            entries.push(Entry {
                act: Act::Space(space.id),
                name: tr!(
                    "browser.command.switch-space",
                    space = &space_name(&space.name)
                )
                .to_string(),
                aliases: "space switch",
            });
        }
    }
    if active.is_some() {
        entries.push(run(
            Command::CloseTab,
            tr!("browser.command.close-tab"),
            "close",
        ));
    }
    if web {
        entries.extend([
            run(Command::Reload, tr!("browser.command.reload"), "refresh"),
            run(
                Command::ZoomIn,
                tr!("browser.command.zoom-in"),
                "zoom larger",
            ),
            run(
                Command::ZoomOut,
                tr!("browser.command.zoom-out"),
                "zoom smaller",
            ),
            run(
                Command::ZoomReset,
                tr!("browser.command.zoom-reset"),
                "zoom actual",
            ),
            run(
                Command::SavePage,
                tr!("browser.command.save-page"),
                "download save pdf print",
            ),
            run(
                Command::Find,
                tr!("browser.command.find"),
                "find search page",
            ),
            run(
                Command::ViewSource,
                tr!("browser.command.view-source"),
                "source html",
            ),
            run(
                Command::Translate,
                match translated {
                    true => tr!("browser.command.show-original"),
                    false => tr!("browser.command.translate"),
                },
                "translate translation language original",
            ),
            run(
                Command::Fullscreen,
                tr!("browser.command.fullscreen"),
                "fullscreen full screen",
            ),
            Entry {
                act: Act::CopyAddress,
                name: tr!("browser.command.copy-url").to_string(),
                aliases: "copy link url address",
            },
        ]);
        if readable {
            entries.push(run(
                Command::ReaderMode,
                tr!("browser.command.reader"),
                "reader read article",
            ));
        }
    }
    if active.is_some() && !private {
        let name = match pinned {
            true => tr!("browser.command.unpin"),
            false => tr!("browser.command.pin"),
        };
        entries.push(run(Command::TogglePin, name, "pin"));
    }
    entries
}

fn run(command: Command, name: Label, aliases: &'static str) -> Entry {
    Entry {
        act: Act::Run(command),
        name: name.to_string(),
        aliases,
    }
}
