mod chosen;
mod command;
mod theme;
mod wallpaper;

pub(crate) use chosen::Chosen;
pub(crate) use command::{Command, CommandInfo};
use futures_util::StreamExt;
use rikka_fuzzy::Score;
use rikka_ui::{MaterialSymbol, tr};
pub(crate) use theme::ThemeCommand;
pub(crate) use wallpaper::WallpaperCommand;

use crate::provider::{Activation, ChangeStream, Hit, Provider, ProviderInfo, Query, Within};

pub(crate) const ID: &str = "commands";

const TRIGGER: &str = "/";

const BY_NAME: f32 = 0.6;

#[derive(Debug)]
pub(super) struct CommandsProvider {
    entries: Vec<Entry>,
    open: Option<Open>,
}

#[derive(Debug)]
struct Entry {
    info: CommandInfo,
    command: Box<dyn Command>,
}

#[derive(Debug)]
struct Open {
    command: usize,
    choices: Vec<Hit>,
}

impl CommandsProvider {
    #[must_use]
    pub(crate) fn new() -> CommandsProvider {
        CommandsProvider {
            entries: Vec::new(),
            open: None,
        }
    }

    #[must_use]
    pub(crate) fn with(mut self, command: impl Command) -> CommandsProvider {
        let command: Box<dyn Command> = Box::new(command);
        let info = command.info();
        self.entries.push(Entry { info, command });
        self
    }

    fn named<'a>(&self, typed: &'a str) -> Option<(usize, &'a str)> {
        let (name, arguments) = typed.split_once(char::is_whitespace)?;
        let command = self
            .entries
            .iter()
            .position(|entry| entry.info.is_named(name))?;
        Some((command, arguments))
    }

    fn rows(&self) -> Vec<Hit> {
        self.entries.iter().map(|entry| entry.info.row()).collect()
    }

    fn enter(&mut self, command: usize) -> &[Hit] {
        let entry = &mut self.entries[command];
        let open = match self.open.take() {
            Some(mut open) if open.command == command => {
                if let Some(choices) = entry.command.reopened() {
                    open.choices = choices;
                }
                open
            }
            _ => Open {
                command,
                choices: entry.command.open(),
            },
        };
        &self.open.insert(open).choices
    }
}

impl Provider for CommandsProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo::new(ID, tr!("shell.launcher.provider.commands"))
            .icon(MaterialSymbol::BOLT)
            .trigger(TRIGGER)
    }

    fn reload(&mut self) {
        self.open = None;
    }

    fn search(&mut self, query: &Query) -> Vec<Hit> {
        if !query.is_forced() {
            return Vec::new();
        }
        let Some((command, arguments)) = self.named(query.typed()) else {
            self.open = None;
            return rank(query, &self.rows());
        };
        rank(&Query::new(arguments), self.enter(command))
    }

    fn activate(&mut self, hit: &Hit) -> Activation {
        let Some(open) = &self.open else {
            return Activation::Ignore;
        };
        self.entries[open.command].command.apply(hit.id())
    }

    fn within(&self) -> Option<Within> {
        let open = self.open.as_ref()?;
        let info = &self.entries[open.command].info;
        Some(Within::new(info.invocation(), info.layout()))
    }

    fn leave(&mut self) {
        self.open = None;
    }

    fn changes(&self) -> Option<ChangeStream> {
        let changes: Vec<ChangeStream> = self
            .entries
            .iter()
            .filter_map(|entry| entry.command.changes())
            .collect();
        (!changes.is_empty()).then(|| futures_util::stream::select_all(changes).boxed())
    }
}

fn rank(query: &Query, offered: &[Hit]) -> Vec<Hit> {
    if query.is_empty() {
        return offered.to_vec();
    }
    offered
        .iter()
        .filter_map(|hit| {
            let score = query.best_of([(hit.title(), 1.0), (hit.id(), BY_NAME)])?;
            (score > Score::NEUTRAL).then(|| hit.clone().score(score))
        })
        .collect()
}
