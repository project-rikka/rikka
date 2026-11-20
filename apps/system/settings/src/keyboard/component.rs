use std::collections::BTreeSet;

use rikka_app::prelude::*;
use rikka_ime::{DEFAULT_LAYOUT, Engine, Fcitx, Group, InputSource};
use rikka_input::Layouts;
use rikka_settings::session::ime;

use crate::domain::{Domain, domain_page};
use crate::keyboard::catalogue::{self, Offer};
use crate::keyboard::pane::Pane;
use crate::keyboard::shortcuts::Shortcuts;
use crate::keyboard::{keymaps, packages};
use crate::page::Page;
use crate::pending::Pending;
use crate::session::Session;
use crate::spot_reveal::SpotReveal;

#[derive(Clone, Debug, Default)]
pub(crate) struct Installed {
    pub(crate) engines: Vec<Engine>,
    pub(crate) group: Group,
}

#[derive(State)]
pub(crate) struct Keyboard {
    pane: Pane,
    domain: Domain,
    layouts: Layouts,
    unreadable: Option<String>,
    engines: Vec<Engine>,
    group: Option<Group>,
    ime_unavailable: Option<String>,
    error: Option<String>,
    following: bool,
    adding: Option<TextDocument>,
    staged: BTreeSet<&'static str>,
    keymaps: Vec<String>,
    shortcuts: Shortcuts,
    reveal: SpotReveal<()>,
}

impl Page for Keyboard {
    fn opened(&mut self, _: &Pending, session: &Session) {
        self.domain.open::<Self>(session);
        self.shortcuts.watch(session);
        if !self.following
            && let Ok(changes) = Fcitx::changes()
        {
            self.following = true;
            stream(changes, |(): ()| read_installed());
        }
        read_installed();
        if self.keymaps.is_empty() {
            spawn_blocking(
                keymaps::installed,
                |keyboard: &mut Keyboard, keymaps: Vec<String>| keyboard.keymaps = keymaps,
            );
        }
        if self.layouts.all().is_empty() && self.unreadable.is_none() {
            spawn_blocking(
                || Layouts::load().map_err(|error| error.to_string()),
                |keyboard: &mut Keyboard, loaded: Result<Layouts, String>| match loaded {
                    Ok(layouts) => keyboard.layouts = layouts,
                    Err(reason) => keyboard.unreadable = Some(reason),
                },
            );
        }
    }

    fn closed(&mut self) {
        self.reveal.clear();
    }
}

domain_page!(Keyboard);

impl Keyboard {
    pub(crate) fn new() -> Self {
        Self {
            pane: Pane::default(),
            domain: Domain::new(ime::DOMAIN),
            layouts: Layouts::default(),
            unreadable: None,
            engines: Vec::new(),
            group: None,
            ime_unavailable: None,
            error: None,
            following: false,
            adding: None,
            staged: BTreeSet::new(),
            keymaps: Vec::new(),
            shortcuts: Shortcuts::new(),
            reveal: SpotReveal::new(),
        }
    }

    pub(crate) fn reveal_sources(&mut self) {
        self.pane = Pane::Input;
        self.adding = None;
        self.reveal.request(());
        self.reveal.settle();
    }

    pub(crate) fn sources_section(&self, section: Container) -> Element {
        self.reveal.section((), section)
    }

    pub(crate) fn pane(&self) -> Pane {
        self.pane
    }

    pub(crate) fn shortcuts(&self) -> &Shortcuts {
        &self.shortcuts
    }

    pub(crate) fn rebuild_finished(&mut self) {
        self.staged.clear();
    }

    pub(crate) fn sources(&self) -> Vec<InputSource> {
        match &self.group {
            Some(group) if !group.members.is_empty() => settings_sources::sources_from_group(group),
            _ => self.sources_to_write(),
        }
    }

    pub(crate) fn layouts(&self) -> &Layouts {
        &self.layouts
    }

    pub(crate) fn keymaps(&self) -> &[String] {
        &self.keymaps
    }

    pub(crate) fn adding(&self) -> Option<&TextDocument> {
        self.adding.as_ref()
    }

    pub(crate) fn query(&self) -> String {
        self.adding
            .as_ref()
            .map(TextDocument::text)
            .unwrap_or_default()
    }

    pub(crate) fn stages(&self, attribute: &str) -> bool {
        self.staged.contains(attribute)
    }

    pub(crate) fn composing(&self) -> Vec<&Engine> {
        let current = self.sources();
        self.engines
            .iter()
            .filter(|engine| !engine.is_plain())
            .filter(|engine| !current.iter().any(|source| source.engine == engine.id))
            .collect()
    }

    pub(crate) fn engine(&self, id: &str) -> Option<&Engine> {
        self.engines.iter().find(|engine| engine.id == id)
    }

    pub(crate) fn describe(&self, source: &InputSource) -> String {
        let layout = self
            .layouts
            .describe(&source.layout, &source.variant)
            .unwrap_or_else(|| source.layout.clone());
        match self.engine(&source.engine) {
            Some(engine) => format!("{} — {layout}", engine.name),
            None if source.engine.is_empty() => layout,
            None => format!("{} — {layout}", source.engine),
        }
    }

    pub(crate) fn unreadable(&self) -> Option<&str> {
        self.unreadable.as_deref()
    }

    pub(crate) fn ime_unavailable(&self) -> Option<&str> {
        self.ime_unavailable.as_deref()
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn show(&mut self, pane: Pane) {
        self.pane = pane;
    }

    pub(crate) fn open_adding(&mut self) {
        self.adding = Some(TextDocument::default());
    }

    pub(crate) fn close_adding(&mut self) {
        self.adding = None;
    }

    fn installed_read(&mut self, installed: Result<Installed, String>) {
        match installed {
            Ok(installed) => self.adopt(installed),
            Err(reason) => {
                self.ime_unavailable = Some(reason);
                self.group = None;
                self.engines.clear();
            }
        }
    }

    pub(crate) fn choose(&mut self, index: usize, pending: &mut Pending) {
        match catalogue::offers(self, pending).get(index) {
            Some(Offer::Layout(name)) => {
                self.adding = None;
                self.add(InputSource::layout(name.clone()));
            }
            Some(Offer::Engine(id)) => {
                let source = self.first_composing_source(id.clone());
                self.adding = None;
                self.add(source);
            }
            Some(Offer::Package(package)) => self.stage(pending, package.attribute),
            Some(Offer::Heading(_)) | None => {}
        }
    }

    fn stage(&mut self, pending: &mut Pending, attribute: &'static str) {
        let installing = !self.staged.contains(attribute);
        match installing {
            true => self.staged.insert(attribute),
            false => self.staged.remove(attribute),
        };
        packages::install(pending, attribute, installing);
    }

    fn sources_to_write(&self) -> Vec<InputSource> {
        self.domain
            .value(ime::SOURCES)
            .and_then(|value| value.get::<Vec<InputSource>>().ok())
            .filter(|sources| !sources.is_empty())
            .unwrap_or_else(|| vec![InputSource::layout(DEFAULT_LAYOUT)])
    }

    fn adopt(&mut self, installed: Installed) {
        self.ime_unavailable = None;
        self.error = None;
        self.engines = installed.engines;
        self.group = Some(installed.group);
        let sources = self.sources();
        if sources != self.sources_to_write() {
            self.write(&sources);
        }
    }

    fn first_composing_source(&self, engine: String) -> InputSource {
        let first = self
            .sources()
            .first()
            .cloned()
            .unwrap_or_else(|| InputSource::layout(DEFAULT_LAYOUT));
        InputSource::layout(first.layout)
            .variant(first.variant)
            .engine(engine)
    }

    fn add(&mut self, source: InputSource) {
        let mut sources = self.sources();
        if sources.contains(&source) {
            return;
        }
        sources.push(source);
        self.commit(&sources);
    }

    pub(crate) fn remove(&mut self, index: usize) {
        let mut sources = self.sources();
        if index >= sources.len() || sources.len() == 1 {
            return;
        }
        sources.remove(index);
        self.commit(&sources);
    }

    pub(crate) fn shift(&mut self, index: usize, step: isize) {
        let mut sources = self.sources();
        let Some(to) = index
            .checked_add_signed(step)
            .filter(|to| *to < sources.len())
        else {
            return;
        };
        sources.swap(index, to);
        self.commit(&sources);
    }

    pub(crate) fn set_variant(&mut self, index: usize, variant: Option<String>) {
        let mut sources = self.sources();
        let Some(source) = sources.get_mut(index) else {
            return;
        };
        source.variant = variant.unwrap_or_default();
        self.commit(&sources);
    }

    fn commit(&mut self, sources: &[InputSource]) {
        self.write(sources);
        let Some(group) = &mut self.group else {
            return;
        };
        *group = settings_sources::group_from_sources(group, sources);
        let group_name = group.clone();
        spawn_blocking(move || set(&group_name), set_ime_error);
    }

    fn write(&mut self, sources: &[InputSource]) {
        let value = rikka_settings::Value::from(&sources)
            .expect("an input source is a table of strings and nothing else");
        self.domain.set_value(ime::SOURCES, value);
    }
}

impl std::fmt::Debug for Keyboard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keyboard")
            .field("pane", &self.pane)
            .field("sources", &self.sources().len())
            .field("layouts", &self.layouts.all().len())
            .field("engines", &self.engines.len())
            .finish_non_exhaustive()
    }
}

pub(crate) fn configure() {
    spawn_blocking(configured, set_ime_error);
}

fn set_ime_error(keyboard: &mut Keyboard, error: Option<String>) {
    keyboard.error = error;
}

fn read_installed() {
    spawn_blocking(
        ask,
        |keyboard: &mut Keyboard, installed: Result<Installed, String>| {
            keyboard.installed_read(installed)
        },
    );
}

fn ask() -> Result<Installed, String> {
    let fcitx = Fcitx::connect().map_err(|error| error.to_string())?;
    let engines = fcitx.engines().map_err(|error| error.to_string())?;
    let group = fcitx.group().map_err(|error| error.to_string())?;
    Ok(Installed { engines, group })
}

fn set(group: &Group) -> Option<String> {
    Fcitx::connect()
        .and_then(|fcitx| fcitx.set_group(group))
        .err()
        .map(|error| error.to_string())
}

fn configured() -> Option<String> {
    Fcitx::connect()
        .and_then(|fcitx| fcitx.configure(""))
        .err()
        .map(|error| error.to_string())
}
