use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use files_core::{Entry, Location, SortKey};
use files_dialog::{Answer, Ask, Kind, OFF, ON, Outcome, Reply};
use files_vfs::Change;
use rikka_app::prelude::*;
use rikka_file_operations::Store;
use rikka_mime::Database;
use rikka_ui::State;

use crate::devices::{self, Devices};
use crate::rename::RenameOutcome;
use crate::services::Services;
use crate::tab::{ChangeSet, Listed, Tab};
use crate::{frame, listing, wording};

#[derive(State)]
pub(crate) struct Dialog {
    pub(crate) ask: Ask,
    pub(crate) services: Services,
    pub(crate) tab: Tab,
    #[state]
    devices: Devices,
    sender: Sender<Dialog>,
    reply: Reply,

    pub(crate) name: TextDocument,
    pub(crate) filter: Option<usize>,
    pub(crate) picks: Vec<String>,
    pub(crate) overwrite: frame::Shown<Location>,
}

impl Dialog {
    pub(crate) fn new(sender: Sender<Dialog>, ask: Ask, mime: Arc<Database>, reply: Reply) -> Self {
        let services =
            Services::attach(&sender, Dialog::thumbnails_arrived, Devices::look, || mime);
        let devices = Devices::new(&services, Dialog::device_command_finished);
        let start = ask
            .folder
            .as_ref()
            .map_or_else(listing::home, Location::from_path);
        let picks = ask
            .choices
            .iter()
            .map(|choice| {
                choice
                    .options
                    .get(choice.initial_index())
                    .map(|option| option.id.clone())
                    .unwrap_or_else(|| if choice.starts_on() { ON } else { OFF }.to_owned())
            })
            .collect();

        Self {
            name: TextDocument::new(&ask.name),
            filter: ask
                .current_filter
                .filter(|index| *index < ask.filters.len()),
            picks,
            ask,
            services,
            tab: Tab::new(0, start),
            devices,
            sender,
            reply,
            overwrite: frame::Shown::default(),
        }
    }

    pub(crate) fn start(&mut self) {
        self.enter_folder();
        self.devices.look();
    }

    fn enter_folder(&mut self) {
        listing::enter_folder(&self.services, &mut self.tab, &self.sender, Dialog::changed);
        self.services.reload_places();
        self.read();
    }

    fn rebuild(&mut self) {
        self.tab.rebuild(&self.services);
        self.tab.refresh_thumbnails(&self.services);
    }

    fn read(&self) {
        listing::read(
            &self.services,
            self.tab.at(),
            self.tab.shows_hidden(),
            Dialog::listed,
        );
    }

    pub(crate) fn refresh(&mut self) {
        self.tab.start_loading();
        self.read();
    }

    fn changed(&mut self, change: Change) {
        if self.tab.record_change(change) {
            after(listing::CHANGE_DEBOUNCE, Dialog::settle);
        }
    }

    fn settle(&mut self) {
        match self.tab.settle() {
            ChangeSet::Listing => self.refresh(),
            ChangeSet::Entries(at) if at.is_empty() => {}
            ChangeSet::Entries(at) => {
                listing::stat_entries(&self.services, at, Dialog::update_entries)
            }
        }
    }

    fn update_entries(&mut self, fresh: Vec<Entry>) {
        if self.tab.update_entries(fresh, &self.services) {
            self.tab.refresh_thumbnails(&self.services);
        }
    }

    pub(crate) fn go(&mut self, to: Location) {
        if self.tab.go(to) {
            self.enter_folder();
        } else {
            self.refresh();
        }
    }

    pub(crate) fn back(&mut self) {
        if self.tab.back() {
            self.enter_folder();
        }
    }

    pub(crate) fn forward(&mut self) {
        if self.tab.forward() {
            self.enter_folder();
        }
    }

    pub(crate) fn up(&mut self) {
        if let Some(at) = self.services.vfs.parent_of(self.tab.at()) {
            self.go(at);
        }
    }

    fn listed(&mut self, mut listed: Listed) {
        if listed.at != *self.tab.at() {
            return;
        }
        listed.entries.retain(|entry| self.keep(entry));
        self.tab.listed(listed, &self.services);
        self.tab.refresh_thumbnails(&self.services);
    }

    fn keep(&self, entry: &Entry) -> bool {
        if entry.kind.is_directory() {
            return true;
        }
        if self.ask.directory {
            return false;
        }
        let Some(filter) = self.filter.and_then(|index| self.ask.filters.get(index)) else {
            return true;
        };
        if filter.rules.is_empty() || filter.matches(&entry.name, &[]) {
            return true;
        }
        let chain = self.services.mime.ancestry(&entry.mime);
        let types: Vec<&str> = chain.iter().map(rikka_mime::MimeType::as_str).collect();
        filter.matches(&entry.name, &types)
    }

    pub(crate) fn open_place(&mut self, index: usize) {
        let Some(row) = self.services.row(index) else {
            return;
        };
        match row.intent() {
            crate::places::Intent::Go(at) => self.go(at),
            crate::places::Intent::Mount(id) => {
                self.devices.run(devices::Command::Mount { id, open: true });
            }
            crate::places::Intent::Unlock(_) | crate::places::Intent::Nothing => {}
        }
    }

    pub(crate) fn open(&mut self, index: usize) {
        let Some(row) = self.tab.rows().get(index) else {
            return;
        };
        if row.is_directory() {
            let at = row.at.clone();
            self.go(at);
            return;
        }
        let at = row.at.clone();
        let name = row.name.clone();
        if self.ask.kind == Kind::Open {
            self.answer(vec![at]);
            return;
        }
        self.name.set_text(name.as_str());
        self.accept();
    }

    pub(crate) fn select(&mut self, chosen: HashSet<usize>, focus: Option<usize>) {
        self.tab.choose(chosen, focus);
        if self.ask.kind == Kind::Save
            && let Some(row) = self.tab.only_selected()
            && !row.is_directory()
        {
            self.name.set_text(row.name.as_str());
        }
    }

    pub(crate) fn sort(&mut self, key: SortKey) {
        self.tab.sort_by(key);
        self.rebuild();
    }

    pub(crate) fn select_all(&mut self) {
        if self.ask.kind != Kind::Open || !self.ask.multiple {
            return;
        }
        self.select(self.tab.all_rows(), self.tab.focus());
    }

    pub(crate) fn escape(&mut self) {
        if self.overwrite.showing().is_some() {
            self.overwrite.dismiss();
        } else if self.tab.editing_path().is_some() {
            self.tab.edit_path(false);
        } else {
            self.cancel();
        }
    }

    pub(crate) fn home(&mut self) {
        self.go(listing::home());
    }

    pub(crate) fn visible(&mut self, range: std::ops::Range<usize>) {
        self.tab.show_range(range, &self.services);
    }

    pub(crate) fn rename_to(&mut self, typed: &str) {
        match self.tab.commit_rename(&self.services.vfs, typed) {
            RenameOutcome::Renamed { .. } => self.refresh(),
            RenameOutcome::Refused(error) => self.fail(wording::failure(&error)),
            RenameOutcome::Nothing => {}
        }
    }

    pub(crate) fn make_folder(&mut self) {
        if self.ask.kind == Kind::Open || !self.tab.folder_allows().create {
            return;
        }
        match self.tab.make(&self.services.vfs, true) {
            Ok(_) => self.refresh(),
            Err(error) => self.fail(wording::failure(&error)),
        }
    }

    fn device_command_finished(&mut self, outcome: devices::Outcome) {
        if let Some(at) = outcome.open.clone() {
            self.go(at);
        }
        if let Some(error) = &outcome.failure {
            self.fail(wording::device(error));
        }
    }

    fn thumbnails_arrived(&mut self, arrived: Vec<Location>) {
        self.tab.thumbnails_arrived(&self.services, arrived);
    }

    pub(crate) fn path(&mut self) {
        let text = self.tab.typed_path();
        match listing::typed(&self.services, &text) {
            Ok(at) => self.go(at),
            Err(reason) => self.fail(reason),
        }
    }

    pub(crate) fn choose_filter(&mut self, index: usize) {
        self.filter = Some(index);
        self.refresh();
    }

    pub(crate) fn pick(&mut self, index: usize, value: String) {
        if let Some(pick) = self.picks.get_mut(index) {
            *pick = value;
        }
    }

    pub(crate) fn toggle_hidden(&mut self) {
        self.tab.toggle_hidden();
        self.refresh();
    }

    pub(crate) fn overwrite(&mut self) {
        let Some(at) = self.overwrite.showing().cloned() else {
            return;
        };
        self.overwrite.dismiss();
        self.answer(vec![at]);
    }

    pub(crate) fn dismiss(&mut self) {
        self.tab.dismiss_failure();
        self.overwrite.dismiss();
    }

    pub(crate) fn cancel(&mut self) {
        self.reply.answer(&Outcome::Cancelled)
    }

    fn fail(&mut self, reason: String) {
        self.tab.fail(reason);
    }

    pub(crate) fn accept(&mut self) {
        match self.ask.kind {
            Kind::Open => self.accept_open(),
            Kind::Save => self.accept_save(),
            Kind::SaveInto => self.accept_save_into(),
        }
    }

    fn accept_open(&mut self) {
        let chosen = self.tab.selected_entries();
        let (folders, mut files): (Vec<Location>, Vec<Location>) =
            (places(&chosen, true), places(&chosen, false));

        if self.ask.directory {
            let chosen = if folders.is_empty() {
                vec![self.tab.at().clone()]
            } else {
                folders
            };
            self.answer(chosen);
            return;
        }

        if files.is_empty()
            && let [at] = folders.as_slice()
        {
            let at = at.clone();
            self.go(at);
            return;
        }
        if files.is_empty() {
            return;
        }
        if !self.ask.multiple {
            files.truncate(1);
        }
        self.answer(files);
    }

    fn accept_save(&mut self) {
        let Some(target) = self.target() else {
            return;
        };
        match self.services.vfs.stat(&target) {
            Ok(entry) if entry.kind.is_directory() => {
                self.name.set_text("");
                self.go(target);
            }
            Ok(_) => self.overwrite.show(target),
            Err(_) => self.answer(vec![target]),
        }
    }

    fn target(&self) -> Option<Location> {
        let written = self.name.text();
        let typed = written.trim();
        if typed.is_empty() {
            return None;
        }
        match typed.strip_prefix("~/") {
            Some(rest) => Some(Location::from_path(rikka_dirs::home()?.join(rest))),
            None => Some(self.tab.at().child(typed)),
        }
    }

    fn accept_save_into(&mut self) {
        let vfs = Arc::clone(&self.services.vfs);
        let here = self.tab.at().clone();
        let mut used_names: HashSet<String> = HashSet::new();
        let chosen: Vec<Location> = self
            .ask
            .names
            .iter()
            .filter_map(|name| {
                let name = PathBuf::from(name);
                let name = name.file_name()?.to_string_lossy().into_owned();
                let free = rikka_format::unique_name(&name, |candidate| {
                    used_names.contains(candidate) || vfs.stat(&here.child(candidate)).is_ok()
                });
                used_names.insert(free.clone());
                Some(here.child(&free))
            })
            .collect();
        if chosen.is_empty() {
            return;
        }
        self.answer(chosen);
    }

    fn answer(&self, chosen: Vec<Location>) {
        let paths: Vec<PathBuf> = chosen.iter().filter_map(Location::to_path).collect();
        if paths.is_empty() {
            return;
        }
        let writable = self.ask.kind != Kind::Open || paths.iter().all(|path| writable(path));
        self.reply.answer(&Outcome::Chosen(Answer {
            chosen: paths,
            filter: self.filter,
            choices: self
                .ask
                .choices
                .iter()
                .zip(&self.picks)
                .map(|(choice, pick)| (choice.id.clone(), pick.clone()))
                .collect(),
            writable,
        }))
    }
}

fn places(chosen: &[&Entry], folders: bool) -> Vec<Location> {
    chosen
        .iter()
        .filter(|entry| entry.kind.is_directory() == folders)
        .map(|entry| entry.location.clone())
        .collect()
}

fn writable(path: &std::path::Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
        && std::fs::OpenOptions::new().write(true).open(path).is_ok()
}

impl std::fmt::Debug for Dialog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dialog")
            .field("kind", &self.ask.kind)
            .field("at", self.tab.at())
            .finish_non_exhaustive()
    }
}
