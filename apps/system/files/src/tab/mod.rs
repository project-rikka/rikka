mod finding;
mod history;
mod pending_changes;
mod thumbnails;

use std::cell::RefCell;
use std::collections::HashSet;
use std::ops::Range;
use std::rc::Rc;

use files_core::{Entry, FolderKind, Location, SCHEME_TRASH, SortKey, Sorting, ViewMode};
use files_thumbs::Thumbnail;
use files_vfs::{Capabilities, Change, Vfs, Watch};
use rikka_i18n::tr;
use rikka_ui::{Label, MaterialSymbol, Place, TextDocument, Watchers};
use rikka_widgets::{ColumnWidths, Tab as Strip, tab};

use crate::browser::{Band, Browser, Row};
use crate::listing;
use crate::rename::{RenameOutcome, Renaming};
use crate::services::Services;

pub(crate) use finding::Finding;
pub(crate) use history::History;
pub(crate) use pending_changes::ChangeSet;
use pending_changes::PendingChanges;

const FIRST_SCREEN: usize = 120;

#[derive(Clone, Debug)]
pub(crate) struct Listed {
    pub(crate) at: Location,
    pub(crate) entries: Vec<Entry>,
    pub(crate) failure: Option<String>,
    pub(crate) kind: FolderKind,
}

#[derive(Clone, Copy, Debug, Default)]
struct Preferred {
    view: Option<ViewMode>,
    sorting: Option<Sorting>,
}

#[derive(Debug)]
pub(crate) struct Tab {
    id: u64,
    at: Location,
    history: History,
    entries: Vec<Entry>,
    rows: Rc<Vec<Row>>,
    thumbnails: Rc<RefCell<Vec<Option<Thumbnail>>>>,
    drawing: Watchers,
    chosen: Rc<HashSet<usize>>,
    scroll: Place,
    widths: ColumnWidths,
    band: Rc<RefCell<Band>>,
    focus: Option<usize>,
    mode: ViewMode,
    sorting: Sorting,
    show_hidden: bool,
    loading: bool,
    capabilities: Capabilities,
    selected: Capabilities,
    finding: Finding,
    renaming: Renaming,
    path_text: TextDocument,
    editing_path: bool,
    pub(crate) failure: Option<String>,
    visible: Range<usize>,
    pending_selection: Vec<Location>,
    preferred: Preferred,
    watch: Option<Watch>,
    pending_changes: PendingChanges,
}

impl Tab {
    pub(crate) fn new(id: u64, at: Location) -> Self {
        Self {
            id,
            at,
            history: History::default(),
            entries: Vec::new(),
            rows: Rc::new(Vec::new()),
            thumbnails: Rc::new(RefCell::new(Vec::new())),
            drawing: Watchers::new(),
            chosen: Rc::new(HashSet::new()),
            scroll: Place::new(),
            widths: ColumnWidths::new(),
            band: Rc::new(RefCell::new(Band::default())),
            focus: None,
            mode: ViewMode::default(),
            sorting: Sorting::new(),
            show_hidden: false,
            loading: true,
            capabilities: Capabilities::read_only(),
            selected: Capabilities::read_only(),
            finding: Finding::default(),
            renaming: Renaming::default(),
            path_text: TextDocument::new(""),
            editing_path: false,
            failure: None,
            visible: 0..FIRST_SCREEN,
            pending_selection: Vec::new(),
            preferred: Preferred::default(),
            watch: None,
            pending_changes: PendingChanges::default(),
        }
    }

    pub(crate) fn adopted(mut self, id: u64) -> Self {
        self.id = id;
        self.watch = None;
        self.pending_changes = PendingChanges::default();
        self
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn at(&self) -> &Location {
        &self.at
    }

    pub(crate) fn history(&self) -> &History {
        &self.history
    }

    pub(crate) fn mode(&self) -> ViewMode {
        self.mode
    }

    pub(crate) fn sorting(&self) -> Sorting {
        self.sorting
    }

    pub(crate) fn shows_hidden(&self) -> bool {
        self.show_hidden
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.loading
    }

    pub(crate) fn folder_allows(&self) -> Capabilities {
        self.capabilities
    }

    pub(crate) fn selection_allows(&self) -> Capabilities {
        self.selected
    }

    pub(crate) fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub(crate) fn finding(&self) -> &Finding {
        &self.finding
    }

    pub(crate) fn finding_mut(&mut self) -> &mut Finding {
        &mut self.finding
    }

    pub(crate) fn editing_path(&self) -> Option<&TextDocument> {
        self.editing_path.then_some(&self.path_text)
    }

    pub(crate) fn go(&mut self, to: Location) -> bool {
        if to == self.at {
            return false;
        }
        let from = std::mem::replace(&mut self.at, to);
        self.history.visit(from);
        true
    }

    pub(crate) fn back(&mut self) -> bool {
        match self.history.back(self.at.clone()) {
            Some(at) => {
                self.at = at;
                true
            }
            None => false,
        }
    }

    pub(crate) fn forward(&mut self) -> bool {
        match self.history.forward(self.at.clone()) {
            Some(at) => {
                self.at = at;
                true
            }
            None => false,
        }
    }

    pub(crate) fn reset_for_navigation(&mut self) {
        self.loading = true;
        self.failure = None;
        self.editing_path = false;
        self.renaming.end();
        self.finding.clear();
        self.scroll.top();
        self.visible = 0..FIRST_SCREEN;
        self.pending_changes = PendingChanges::default();
    }

    pub(crate) fn set_watch(&mut self, watch: Option<Watch>) {
        self.watch = watch;
    }

    pub(crate) fn record_change(&mut self, change: Change) -> bool {
        self.pending_changes.record_change(change)
    }

    pub(crate) fn settle(&mut self) -> ChangeSet {
        self.pending_changes.settle()
    }

    pub(crate) fn update_entries(&mut self, fresh: Vec<Entry>, services: &Services) -> bool {
        let mut changed = false;
        for mut entry in fresh {
            let Some(known) = self
                .entries
                .iter_mut()
                .find(|known| known.location == entry.location)
            else {
                continue;
            };
            entry.hidden = known.hidden;
            *known = entry;
            changed = true;
        }
        if changed {
            self.rebuild(services);
        }
        changed
    }

    pub(crate) fn start_loading(&mut self) {
        self.loading = true;
    }

    pub(crate) fn stop_loading(&mut self) {
        self.loading = false;
    }

    pub(crate) fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
    }

    pub(crate) fn edit_path(&mut self, editing: bool) {
        self.editing_path = editing;
        if editing {
            let here = self.at.display();
            self.path_text.set_text(here);
            self.path_text.select_all();
        }
    }

    pub(crate) fn typed_path(&mut self) -> String {
        self.editing_path = false;
        self.path_text.text()
    }

    pub(crate) fn fail(&mut self, reason: String) {
        self.failure = Some(reason);
    }

    pub(crate) fn dismiss_failure(&mut self) {
        self.failure = None;
    }

    pub(crate) fn select_on_arrival(&mut self, locations: Vec<Location>) {
        self.pending_selection = locations;
    }

    pub(crate) fn listed(&mut self, listed: Listed, services: &Services) -> bool {
        self.adopt(listed);
        self.capabilities = services.vfs.capabilities(&self.at);
        self.rebuild(services);
        self.renaming.follow(&self.rows);
        self.restore_selection();
        if self.chosen.is_empty() {
            self.selected = Capabilities::read_only();
            return false;
        }
        true
    }

    fn adopt(&mut self, listed: Listed) {
        self.entries = listed.entries;
        if listed.failure.is_some() {
            self.failure = listed.failure;
        }
        self.mode = self.preferred.view.unwrap_or_else(|| listed.kind.view());
        self.sorting = self
            .preferred
            .sorting
            .unwrap_or_else(|| listed.kind.sorting());
        self.loading = false;
    }

    fn restore_selection(&mut self) {
        let pending = std::mem::take(&mut self.pending_selection);
        if pending.is_empty() {
            return;
        }
        let chosen: HashSet<usize> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| pending.contains(&row.at))
            .map(|(index, _)| index)
            .collect();
        self.focus = chosen.iter().copied().min();
        self.chosen = Rc::new(chosen);
    }

    pub(crate) fn rebuild(&mut self, services: &Services) {
        services
            .mime
            .set_language(&rikka_i18n::current_locale().to_string());
        let rows: Vec<Row> = self
            .entries
            .iter()
            .map(|entry| Row::build(entry, listing::describe(services, entry)))
            .collect();
        self.reorder(rows);
    }

    fn reorder(&mut self, rows: Vec<Row>) {
        let retained: HashSet<Location> = self
            .chosen
            .iter()
            .filter_map(|index| self.rows.get(*index).map(|row| row.at.clone()))
            .collect();
        let focused = self
            .focus
            .and_then(|index| self.rows.get(index))
            .map(|row| row.at.clone());

        let mut order: Vec<usize> = (0..rows.len().min(self.entries.len())).collect();
        let sorting = self.sorting;
        order.sort_by(|&one, &other| sorting.cmp(&self.entries[one], &self.entries[other]));

        let mut sorted_rows = Vec::with_capacity(order.len());
        let mut sorted_entries = Vec::with_capacity(order.len());
        let mut rows: Vec<Option<Row>> = rows.into_iter().map(Some).collect();
        let mut entries: Vec<Option<Entry>> = std::mem::take(&mut self.entries)
            .into_iter()
            .map(Some)
            .collect();
        for index in order {
            if let (Some(row), Some(entry)) = (rows[index].take(), entries[index].take()) {
                sorted_rows.push(row);
                sorted_entries.push(entry);
            }
        }

        self.chosen = Rc::new(
            sorted_rows
                .iter()
                .enumerate()
                .filter(|(_, row)| retained.contains(&row.at))
                .map(|(index, _)| index)
                .collect(),
        );
        self.focus = focused.and_then(|at| sorted_rows.iter().position(|row| row.at == at));
        self.entries = sorted_entries;
        self.thumbnails = Rc::new(RefCell::new(vec![None; sorted_rows.len()]));
        self.rows = Rc::new(sorted_rows);
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.chosen = Rc::new(HashSet::new());
        self.focus = None;
    }

    pub(crate) fn append(&mut self, more: Vec<Entry>) {
        self.entries.extend(more);
    }

    pub(crate) fn choose(&mut self, chosen: HashSet<usize>, focus: Option<usize>) {
        self.chosen = Rc::new(chosen);
        self.focus = focus;
    }

    pub(crate) fn all_rows(&self) -> HashSet<usize> {
        (0..self.rows.len()).collect()
    }

    pub(crate) fn focus(&self) -> Option<usize> {
        self.focus
    }

    pub(crate) fn recheck_selection(&mut self, vfs: &Vfs) {
        let first = self.selection().first().cloned();
        self.selected = first.map_or_else(Capabilities::read_only, |at| vfs.capabilities(&at));
    }

    pub(crate) fn chosen_count(&self) -> usize {
        self.chosen.len()
    }

    pub(crate) fn first_chosen(&self) -> Option<usize> {
        self.chosen.iter().copied().min()
    }

    fn chosen_in_order(&self) -> Vec<usize> {
        let mut chosen: Vec<usize> = self.chosen.iter().copied().collect();
        chosen.sort_unstable();
        chosen
    }

    pub(crate) fn selection(&self) -> Vec<Location> {
        self.chosen_in_order()
            .into_iter()
            .filter_map(|index| self.rows.get(index).map(|row| row.at.clone()))
            .collect()
    }

    pub(crate) fn everything(&self) -> Vec<Location> {
        self.rows.iter().map(|row| row.at.clone()).collect()
    }

    pub(crate) fn only_selected(&self) -> Option<&Row> {
        if self.chosen.len() != 1 {
            return None;
        }
        self.rows.get(*self.chosen.iter().next()?)
    }

    pub(crate) fn selected_entries(&self) -> Vec<&Entry> {
        self.chosen_in_order()
            .into_iter()
            .filter_map(|index| self.entries.get(index))
            .collect()
    }

    pub(crate) fn selected_folders(&self) -> Vec<Location> {
        self.selected_entries()
            .into_iter()
            .filter(|entry| entry.is_directory())
            .map(|entry| entry.location.clone())
            .collect()
    }

    pub(crate) fn any_folder_selected(&self) -> bool {
        self.chosen
            .iter()
            .filter_map(|index| self.entries.get(*index))
            .any(Entry::is_directory)
    }

    pub(crate) fn entry(&self, index: usize) -> Option<&Entry> {
        self.entries.get(index)
    }

    pub(crate) fn index_of(&self, at: &Location) -> Option<usize> {
        self.rows.iter().position(|row| row.at == *at)
    }

    pub(crate) fn begin_rename(&mut self) {
        let Some(row) = self.only_selected().filter(|_| self.capabilities.rename) else {
            return;
        };
        let (at, name, folder) = (row.at.clone(), row.name.clone(), row.is_directory());
        self.renaming.begin(at, &name, folder);
    }

    pub(crate) fn end_rename(&mut self) {
        self.renaming.end();
    }

    pub(crate) fn commit_rename(&mut self, vfs: &Vfs, typed: &str) -> RenameOutcome {
        self.renaming.commit(vfs, typed)
    }

    pub(crate) fn make(&mut self, vfs: &Vfs, folder: bool) -> Result<Location, files_vfs::Error> {
        self.renaming.make(vfs, &self.at, folder)
    }

    pub(crate) fn view(&mut self, mode: ViewMode) {
        self.mode = mode;
        self.preferred.view = Some(mode);
    }

    pub(crate) fn sort_by(&mut self, key: SortKey) {
        self.sorting = self.sorting.toggled(key);
        self.preferred.sorting = Some(self.sorting);
    }

    pub(crate) fn browser(&self) -> Browser {
        Browser::new(
            Rc::clone(&self.rows),
            Rc::clone(&self.thumbnails),
            self.drawing.clone(),
            self.scroll.clone(),
            self.widths.clone(),
            Rc::clone(&self.band),
        )
        .showing(self.mode, self.sorting)
        .chosen(Rc::clone(&self.chosen), self.focus)
        .renaming(self.renaming.showing(self))
    }

    pub(crate) fn strip(&self) -> Strip {
        let trash = self.at.scheme() == SCHEME_TRASH;
        let here = self.at.path();
        let label = match here.file_name() {
            Some(name) => Label::from(name.to_string_lossy().into_owned()),
            None if trash => tr!("files.places.trash"),
            None if self.at.is_local() => Label::from("/"),
            None => Label::from(self.at.scheme()),
        };
        tab(label).icon(if trash {
            MaterialSymbol::DELETE
        } else {
            MaterialSymbol::FOLDER
        })
    }
}
