mod browser;
mod location;
mod menus;
mod row;
mod transfer;
pub(crate) mod view;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use futures_util::stream::{self, StreamExt};
use google_drive::{Changes, Error, File, Storage, Thumbnail};
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse};
use rikka_ui::{DragAction, DragOffer, clipboard};

use crate::session::Session;
use browser::{Band, Browser, ITEMS, Rename, Thumbnails};
use location::{Crumb, Location, Root};
use row::{Column, Row, Sorting, ViewMode};
use transfer::{Job, Transfer};

const LISTING: &str = "google-drive-listing";
const CHANGES: &str = "google-drive-changes";
const CHANGES_PERIOD: Duration = Duration::from_secs(30);
const THUMBNAIL: u32 = 220;
const AT_ONCE: usize = 6;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Listing {
    Loading,
    Ready,
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Menu {
    Context(Point),
    New,
    Sort,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Asking {
    DeleteForever(Vec<String>),
    EmptyTrash,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Notice {
    pub(crate) text: String,
    pub(crate) restores: Vec<String>,
    pub(crate) danger: bool,
    pub(crate) folder: Option<PathBuf>,
}

#[derive(State)]
pub(crate) struct DriveState {
    #[untracked]
    session: Option<Arc<Session>>,
    location: Tracked<Location>,
    #[untracked]
    back: Vec<Location>,
    #[untracked]
    forward: Vec<Location>,
    #[untracked]
    era: u64,
    #[untracked]
    visits: u64,
    #[untracked]
    rows: Rc<Vec<Row>>,
    #[untracked]
    chosen: Rc<HashSet<usize>>,
    #[untracked]
    focus: Option<usize>,
    #[untracked]
    renaming: Option<Rename>,
    #[untracked]
    renames: u64,
    shown: Revision,
    listing: Tracked<Listing>,
    mode: Tracked<ViewMode>,
    sorting: Tracked<Sorting>,
    #[untracked]
    search: TextDocument,
    seeking: Tracked<bool>,
    menu: Tracked<Option<(Menu, u64)>>,
    asking: Tracked<Option<Asking>>,
    transfer: Tracked<Option<Transfer>>,
    #[untracked]
    queue: VecDeque<Job>,
    notice: Tracked<Option<Notice>>,
    storage: Tracked<Option<Storage>>,
    signed_out: Tracked<bool>,
    #[untracked]
    thumbnails: Thumbnails,
    #[untracked]
    wanted: HashSet<String>,
    #[untracked]
    drawing: Watchers,
    #[untracked]
    scroll: Place,
    #[untracked]
    widths: ColumnWidths,
    #[untracked]
    band: Rc<RefCell<Band>>,
    #[untracked]
    visible: Range<usize>,
    #[untracked]
    cursor: Option<String>,
    #[untracked]
    watched: bool,
}

impl Default for DriveState {
    fn default() -> DriveState {
        DriveState {
            session: None,
            location: Tracked::new(Location::root(Root::MyDrive)),
            back: Vec::new(),
            forward: Vec::new(),
            era: 0,
            visits: 0,
            rows: Rc::new(Vec::new()),
            chosen: Rc::new(HashSet::new()),
            focus: None,
            renaming: None,
            renames: 0,
            shown: Revision::new(),
            listing: Tracked::new(Listing::Loading),
            mode: Tracked::new(ViewMode::Details),
            sorting: Tracked::new(Sorting::default()),
            search: TextDocument::new(""),
            seeking: Tracked::new(false),
            menu: Tracked::new(None),
            asking: Tracked::new(None),
            transfer: Tracked::new(None),
            queue: VecDeque::new(),
            notice: Tracked::new(None),
            storage: Tracked::new(None),
            signed_out: Tracked::new(false),
            thumbnails: Rc::new(RefCell::new(HashMap::new())),
            wanted: HashSet::new(),
            drawing: Watchers::default(),
            scroll: Place::default(),
            widths: ColumnWidths::new(),
            band: Rc::new(RefCell::new(Band::default())),
            visible: 0..0,
            cursor: None,
            watched: false,
        }
    }
}

impl DriveState {
    pub(crate) fn location(&self) -> &Location {
        &self.location
    }

    pub(crate) fn listing(&self) -> &Listing {
        &self.listing
    }

    pub(crate) fn mode(&self) -> ViewMode {
        *self.mode
    }

    pub(crate) fn sorting(&self) -> Sorting {
        *self.sorting
    }

    pub(crate) fn search(&self) -> &TextDocument {
        &self.search
    }

    pub(crate) fn is_seeking(&self) -> bool {
        *self.seeking
    }

    pub(crate) fn menu(&self) -> Option<(Menu, u64)> {
        *self.menu
    }

    pub(crate) fn asking(&self) -> Option<&Asking> {
        self.asking.as_ref()
    }

    pub(crate) fn transfer(&self) -> Option<&Transfer> {
        self.transfer.as_ref()
    }

    pub(crate) fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    pub(crate) fn storage(&self) -> Option<&Storage> {
        self.storage.as_ref()
    }

    pub(crate) fn is_signed_out(&self) -> bool {
        *self.signed_out
    }

    pub(crate) fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    pub(crate) fn count(&self) -> usize {
        self.shown.read();
        self.rows.len()
    }

    pub(crate) fn selected(&self) -> Vec<&Row> {
        self.shown.read();
        let mut chosen: Vec<usize> = self.chosen.iter().copied().collect();
        chosen.sort_unstable();
        chosen
            .into_iter()
            .filter_map(|index| self.rows.get(index))
            .collect()
    }

    pub(crate) fn depth(&self) -> usize {
        self.back.len()
    }
    pub(crate) fn page(&self) -> u64 {
        self.visits
    }

    pub(crate) fn takes_files(&self) -> bool {
        self.location.folder().is_some()
    }

    pub(crate) fn browser(&self) -> Browser {
        self.shown.read();
        Browser::new(
            Rc::clone(&self.rows),
            Rc::clone(&self.thumbnails),
            self.drawing.clone(),
            self.scroll.clone(),
            self.widths.clone(),
            Rc::clone(&self.band),
        )
        .showing(*self.mode, *self.sorting)
        .chosen(Rc::clone(&self.chosen), self.focus)
        .renaming(self.renaming.clone())
    }

    pub(crate) fn attach(&mut self, session: Arc<Session>) {
        self.detach();
        let drive = session.drive.clone();
        let shelf = session.shelf.clone();
        self.session = Some(session);
        self.load(false);
        self.refresh_storage();
        spawn(
            async move { drive.start_cursor().await },
            DriveState::cursor_started,
        );
        spawn_blocking(
            move || shelf.prune_opened(transfer::OPENED_BUDGET),
            |pruned: google_drive::Result<()>| {
                if let Err(error) = pruned {
                    tracing::warn!("failed to prune opened Drive files: {error}");
                }
                skip_view();
            },
        );
        every_keyed(CHANGES, CHANGES_PERIOD, DriveState::poll_changes);
    }

    pub(crate) fn detach(&mut self) {
        cancel(LISTING);
        cancel(CHANGES);
        cancel(transfer::RUNNING);
        cancel(transfer::PROGRESS);
        if let Some(transfer) = self.transfer.as_ref() {
            transfer.cancel.cancel();
        }
        *self = DriveState::default();
    }

    pub(crate) fn watch(&mut self, watched: bool) {
        self.watched = watched;
        if watched {
            self.poll_changes();
        }
    }

    pub(crate) fn go(&mut self, location: Location) {
        if *self.location == location {
            return;
        }
        let left = std::mem::replace(&mut *self.location, location);
        self.back.push(left);
        self.forward.clear();
        self.arrived();
    }

    pub(crate) fn go_root(&mut self, index: usize) {
        if let Some(root) = Root::ALL.get(index) {
            self.go(Location::root(*root));
        }
    }

    pub(crate) fn go_crumb(&mut self, depth: usize) {
        self.go(self.location.ancestor(depth));
    }

    pub(crate) fn back(&mut self) {
        if let Some(previous) = self.back.pop() {
            let left = std::mem::replace(&mut *self.location, previous);
            self.forward.push(left);
            self.arrived();
        }
    }

    pub(crate) fn forward(&mut self) {
        if let Some(next) = self.forward.pop() {
            let left = std::mem::replace(&mut *self.location, next);
            self.back.push(left);
            self.arrived();
        }
    }

    pub(crate) fn up(&mut self) {
        if let Some(parent) = self.location.parent() {
            self.go(parent);
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.load(true);
        self.refresh_storage();
    }

    pub(crate) fn seek(&mut self) {
        self.seeking.set(true);
    }

    pub(crate) fn seek_ended(&mut self) {
        self.seeking.set(false);
    }
    pub(crate) fn sought(&mut self) {
        self.seeking.set(false);
        let text = self.search.text();
        let text = text.trim();
        if !text.is_empty() {
            self.go(Location::search(text));
        }
    }

    pub(crate) fn view_as(&mut self, mode: ViewMode) {
        self.mode.set(mode);
        self.fetch_thumbnails();
    }

    pub(crate) fn sort_by(&mut self, column: Column) {
        let sorting = self.sorting.toggled(column);
        self.sorting.set(sorting);
        self.arrange(self.rows.iter().map(|row| row.file.clone()).collect());
    }

    pub(crate) fn sort_as_listed(&mut self) {
        self.sorting.set(Sorting::default());
        self.load(true);
    }

    pub(crate) fn select(&mut self, chosen: HashSet<usize>, focus: Option<usize>) {
        if *self.chosen == chosen && self.focus == focus {
            skip_view();
            return;
        }
        self.chosen = Rc::new(chosen);
        self.focus = focus;
        self.shown.changed();
    }

    pub(crate) fn select_all(&mut self) {
        self.chosen = Rc::new((0..self.rows.len()).collect());
        self.shown.changed();
    }

    pub(crate) fn visible(&mut self, range: Range<usize>) {
        self.visible = range;
        self.fetch_thumbnails();
        skip_view();
    }

    pub(crate) fn show_menu(&mut self, menu: Menu) {
        let key = self.menu.map_or(1, |(_, key)| key + 1);
        self.menu.set(Some((menu, key)));
    }

    pub(crate) fn close_menu(&mut self) {
        self.menu.set(None);
    }

    pub(crate) fn dismiss_notice(&mut self) {
        self.notice.set(None);
    }

    pub(crate) fn open(&mut self, index: usize) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let file = row.file.clone();
        if file.is_folder() {
            self.go(self.location.entered(Crumb {
                id: file.target_id().to_owned(),
                name: file.name,
            }));
        } else if file.is_native() {
            self.open_in_browser(&file);
        } else {
            self.enqueue(Job::Open(Box::new(file)));
        }
    }

    pub(crate) fn open_selected(&mut self) {
        self.close_menu();
        if let Some(index) = self.focus.or_else(|| self.chosen.iter().min().copied()) {
            self.open(index);
        }
    }

    pub(crate) fn browse_selected(&mut self) {
        self.close_menu();
        let files: Vec<File> = self
            .selected()
            .into_iter()
            .map(|row| row.file.clone())
            .collect();
        for file in &files {
            self.open_in_browser(file);
        }
    }

    pub(crate) fn copy_link(&mut self) {
        self.close_menu();
        let links: Vec<&str> = self
            .selected()
            .into_iter()
            .filter_map(|row| row.file.web_view_link.as_deref())
            .collect();
        clipboard::set_text(&links.join("\n"));
    }

    pub(crate) fn download_selected(&mut self, pdf: bool) {
        self.close_menu();
        let files: Vec<File> = self
            .selected()
            .into_iter()
            .map(|row| row.file.clone())
            .filter(|file| !file.is_folder())
            .collect();
        if files.is_empty() {
            return;
        }
        let parent = window_identifier();
        spawn_blocking(
            move || {
                Chooser::open(tr!("google.drive.download.title").to_string())
                    .parent(parent)
                    .directory(true)
                    .accept_label(tr!("google.drive.download.accept").to_string())
                    .run()
            },
            move |drive: &mut DriveState, chosen: rikka_dialog::Result<ChooserResponse>| {
                if let Some(folder) = transfer::chosen_paths(chosen).into_iter().next() {
                    drive.enqueue(Job::Download {
                        files: files.clone(),
                        folder,
                        pdf,
                    });
                }
            },
        );
    }

    pub(crate) fn upload(&mut self) {
        self.close_menu();
        let Some(parent) = self.location.folder().map(str::to_owned) else {
            return;
        };
        let window = window_identifier();
        spawn_blocking(
            move || {
                Chooser::open(tr!("google.drive.upload.title").to_string())
                    .parent(window)
                    .multiple(true)
                    .accept_label(tr!("google.drive.upload.accept").to_string())
                    .run()
            },
            move |drive: &mut DriveState, chosen: rikka_dialog::Result<ChooserResponse>| {
                let paths = transfer::chosen_paths(chosen);
                if !paths.is_empty() {
                    drive.enqueue(Job::Upload {
                        paths,
                        parent: parent.clone(),
                    });
                }
            },
        );
    }

    pub(crate) fn dropped(&mut self, into: Option<usize>, offer: DragOffer, _: DragAction) {
        let target = match into {
            Some(index) => self
                .rows
                .get(index)
                .map(|row| row.file.target_id().to_owned()),
            None => self.location.folder().map(str::to_owned),
        };
        let Some(target) = target else {
            return;
        };
        if offer.is_ours() && offer.offers(ITEMS) {
            let ids = offer
                .read(ITEMS)
                .map(|ids| String::from_utf8_lossy(&ids).into_owned())
                .unwrap_or_default();
            self.relocate(ids.lines().map(str::to_owned).collect(), target);
            return;
        }
        let paths: Vec<PathBuf> = offer
            .uris()
            .iter()
            .filter_map(|uri| rikka_open::uri_to_path(uri))
            .filter(|path| path.is_file())
            .collect();
        if !paths.is_empty() {
            self.enqueue(Job::Upload {
                paths,
                parent: target,
            });
        }
    }

    pub(crate) fn new_folder(&mut self) {
        self.close_menu();
        let (Some(session), Some(parent)) = (&self.session, self.location.folder()) else {
            return;
        };
        let drive = session.drive.clone();
        let parent = parent.to_owned();
        let name = tr!("google.drive.new-folder.name").to_string();
        let era = self.era;
        spawn(
            async move { drive.create_folder(&parent, &name).await },
            move |drive: &mut DriveState, created: google_drive::Result<File>| match created {
                Ok(folder) if drive.era == era => {
                    let id = folder.id.clone();
                    let mut files: Vec<File> =
                        drive.rows.iter().map(|row| row.file.clone()).collect();
                    files.insert(0, folder);
                    drive.arrange(files);
                    if let Some(index) = drive.rows.iter().position(|row| row.file.id == id) {
                        drive.select(HashSet::from([index]), Some(index));
                        drive.begin_rename();
                    }
                }
                Ok(_) => skip_view(),
                Err(error) => drive.failed(&error),
            },
        );
    }

    pub(crate) fn begin_rename(&mut self) {
        self.close_menu();
        let Some(index) = self.focus.filter(|index| self.chosen.contains(index)) else {
            return;
        };
        let Some(row) = self.rows.get(index) else {
            return;
        };
        if !row.file.capabilities.can_rename && row.file.drive_id.is_none() {
            return;
        }
        self.renames += 1;
        let document = TextDocument::new(&row.file.name);
        let stem = match row.file.is_folder() {
            true => row.file.name.len(),
            false => rikka_format::name_stem(&row.file.name).len(),
        };
        document.select(0, stem);
        self.renaming = Some(Rename {
            index,
            document,
            session: self.renames,
        });
        self.shown.changed();
    }

    pub(crate) fn end_rename(&mut self) {
        if self.renaming.take().is_some() {
            self.shown.changed();
        }
    }

    pub(crate) fn rename_to(&mut self, name: &str) {
        let Some(rename) = self.renaming.take() else {
            skip_view();
            return;
        };
        self.shown.changed();
        let name = name.trim().to_owned();
        let (Some(session), Some(row)) = (&self.session, self.rows.get(rename.index)) else {
            return;
        };
        if name.is_empty() || name == row.file.name {
            return;
        }
        let drive = session.drive.clone();
        let id = row.file.id.clone();
        spawn(
            async move { vec![drive.rename(&id, &name).await] },
            DriveState::changed,
        );
    }

    pub(crate) fn star_selected(&mut self, starred: bool) {
        self.close_menu();
        let Some(session) = &self.session else {
            return;
        };
        let ids: Vec<String> = self
            .selected()
            .into_iter()
            .map(|row| row.file.id.clone())
            .collect();
        let drive = session.drive.clone();
        spawn(
            async move {
                stream::iter(ids)
                    .map(|id| {
                        let drive = &drive;
                        async move { drive.set_starred(&id, starred).await }
                    })
                    .buffer_unordered(AT_ONCE)
                    .collect::<Vec<_>>()
                    .await
            },
            DriveState::changed,
        );
    }

    pub(crate) fn discard_selected(&mut self) {
        self.close_menu();
        let ids: Vec<String> = self
            .selected()
            .into_iter()
            .map(|row| row.file.id.clone())
            .collect();
        if ids.is_empty() {
            return;
        }
        match self.location.is_trash() {
            true => self.asking.set(Some(Asking::DeleteForever(ids))),
            false => self.trash(ids, true),
        }
    }

    pub(crate) fn restore_selected(&mut self) {
        self.close_menu();
        let ids = self
            .selected()
            .into_iter()
            .map(|row| row.file.id.clone())
            .collect();
        self.trash(ids, false);
    }

    pub(crate) fn undo_notice(&mut self) {
        if let Some(notice) = self.notice.take() {
            self.trash(notice.restores, false);
        }
    }

    pub(crate) fn ask_empty_trash(&mut self) {
        self.close_menu();
        self.asking.set(Some(Asking::EmptyTrash));
    }

    pub(crate) fn answer(&mut self, confirmed: bool) {
        let (Some(asking), Some(session)) = (self.asking.take(), &self.session) else {
            return;
        };
        if !confirmed {
            return;
        }
        let drive = session.drive.clone();
        spawn(
            async move {
                match asking {
                    Asking::EmptyTrash => drive.empty_trash().await,
                    Asking::DeleteForever(ids) => {
                        every(ids.into_iter().map(|id| {
                            let drive = &drive;
                            async move { drive.delete(&id).await }
                        }))
                        .await
                    }
                }
            },
            DriveState::settled,
        );
    }

    fn trash(&mut self, ids: Vec<String>, trashed: bool) {
        let Some(session) = self.session.clone() else {
            return;
        };
        if ids.is_empty() {
            return;
        }
        let kept: Vec<File> = self
            .rows
            .iter()
            .filter(|row| !ids.contains(&row.file.id))
            .map(|row| row.file.clone())
            .collect();
        self.arrange(kept);
        self.notice.set(trashed.then(|| Notice {
            text: tr!("google.drive.trashed", count = ids.len()).to_string(),
            restores: ids.clone(),
            danger: false,
            folder: None,
        }));
        let drive = session.drive.clone();
        spawn(
            async move {
                every(ids.into_iter().map(|id| {
                    let drive = &drive;
                    async move { drive.set_trashed(&id, trashed).await.map(drop) }
                }))
                .await
            },
            DriveState::settled,
        );
    }

    fn relocate(&mut self, ids: Vec<String>, target: String) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let moved: Vec<(String, Vec<String>)> = self
            .rows
            .iter()
            .filter(|row| ids.contains(&row.file.id) && row.file.id != target)
            .map(|row| (row.file.id.clone(), row.file.parents.clone()))
            .collect();
        if moved.is_empty() {
            return;
        }
        let kept: Vec<File> = self
            .rows
            .iter()
            .filter(|row| !moved.iter().any(|(id, _)| *id == row.file.id))
            .map(|row| row.file.clone())
            .collect();
        self.arrange(kept);
        let drive = session.drive.clone();
        spawn(
            async move {
                every(moved.into_iter().map(|(id, parents)| {
                    let (drive, target) = (&drive, &target);
                    async move { drive.relocate(&id, &parents, target).await.map(drop) }
                }))
                .await
            },
            DriveState::settled,
        );
    }

    fn open_in_browser(&mut self, file: &File) {
        let Some(link) = &file.web_view_link else {
            return;
        };
        if let Err(error) = rikka_open::open(link) {
            self.notify(error.to_string());
        }
    }

    fn changed(&mut self, changed: Vec<google_drive::Result<File>>) {
        let mut files: Vec<File> = self.rows.iter().map(|row| row.file.clone()).collect();
        let mut replaced = false;
        let mut failure = None;
        for change in changed {
            match change {
                Ok(file) => {
                    if let Some(known) = files.iter_mut().find(|known| known.id == file.id) {
                        *known = file;
                        replaced = true;
                    }
                }
                Err(error) => failure = Some(error),
            }
        }
        if replaced {
            self.arrange(files);
        }
        match failure {
            Some(error) => self.failed(&error),
            None if !replaced => skip_view(),
            None => {}
        }
    }

    fn settled(&mut self, settled: google_drive::Result<()>) {
        match settled {
            Ok(()) => self.refresh_storage(),
            Err(error) => self.failed(&error),
        }
        self.load(true);
    }

    fn failed(&mut self, error: &Error) {
        if error.is_signed_out() {
            self.signed_out.set(true);
        }
        self.notify(view::failure(error));
    }

    fn notify(&mut self, text: String) {
        self.notice.set(Some(Notice {
            text,
            restores: Vec::new(),
            danger: true,
            folder: None,
        }));
    }

    fn arrived(&mut self) {
        self.visits += 1;
        self.search.clear();
        self.scroll = Place::default();
        self.load(false);
    }

    fn load(&mut self, quietly: bool) {
        let Some(session) = self.session.clone() else {
            return;
        };
        self.era += 1;
        let scope = self.location.scope();
        if !quietly {
            self.renaming = None;
            self.chosen = Rc::new(HashSet::new());
            self.focus = None;
            let recalled = session.shelf.recall(&scope).unwrap_or_else(|error| {
                tracing::warn!("failed to read the kept Drive listing: {error}");
                None
            });
            match recalled {
                Some(files) => {
                    self.arrange(files);
                    self.listing.set(Listing::Ready);
                }
                None => {
                    self.rows = Rc::new(Vec::new());
                    self.shown.changed();
                    self.listing.set(Listing::Loading);
                }
            }
        }
        let drive = session.drive.clone();
        let shelf = session.shelf.clone();
        let era = self.era;
        spawn_keyed(
            LISTING,
            async move {
                let listed = drive.list_all(&scope).await;
                if let Ok(files) = &listed
                    && let Err(error) = shelf.remember(&scope, files)
                {
                    tracing::warn!("failed to keep the Drive listing: {error}");
                }
                listed
            },
            move |drive: &mut DriveState, listed: google_drive::Result<Vec<File>>| {
                if drive.era != era {
                    return skip_view();
                }
                match listed {
                    Ok(files) => {
                        drive.arrange(files);
                        drive.listing.set(Listing::Ready);
                    }
                    Err(error) => {
                        if error.is_signed_out() {
                            drive.signed_out.set(true);
                        }
                        if drive.rows.is_empty() {
                            drive.listing.set(Listing::Failed(view::failure(&error)));
                        } else {
                            drive.notify(view::failure(&error));
                        }
                    }
                }
            },
        );
    }

    fn arrange(&mut self, files: Vec<File>) {
        let chosen: HashSet<&str> = self
            .chosen
            .iter()
            .filter_map(|index| self.rows.get(*index))
            .map(|row| row.file.id.as_str())
            .collect();
        let focused = self
            .focus
            .and_then(|index| self.rows.get(index))
            .map(|row| row.file.id.clone());
        let renamed = self
            .renaming
            .as_ref()
            .and_then(|rename| self.rows.get(rename.index))
            .map(|row| row.file.id.clone());
        let mut rows: Vec<Row> = files.into_iter().map(Row::new).collect();
        self.sorting.sort(&mut rows);
        let position = |id: &Option<String>| {
            id.as_ref()
                .and_then(|id| rows.iter().position(|row| row.file.id == *id))
        };
        let kept: HashSet<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| chosen.contains(row.file.id.as_str()))
            .map(|(index, _)| index)
            .collect();
        let focus = position(&focused);
        match (position(&renamed), &mut self.renaming) {
            (Some(index), Some(rename)) => rename.index = index,
            _ => self.renaming = None,
        }
        self.focus = focus;
        self.chosen = Rc::new(kept);
        self.rows = Rc::new(rows);
        self.shown.changed();
        self.fetch_thumbnails();
    }

    fn refresh_storage(&mut self) {
        let Some(session) = &self.session else {
            return;
        };
        let drive = session.drive.clone();
        spawn(
            async move { drive.storage().await },
            |drive: &mut DriveState, storage: google_drive::Result<Storage>| match storage {
                Ok(storage) => drive.storage.set(Some(storage)),
                Err(error) => {
                    tracing::debug!("failed to read the Drive quota: {error}");
                    skip_view();
                }
            },
        );
    }

    fn poll_changes(&mut self) {
        skip_view();
        let (Some(session), true) = (&self.session, self.watched) else {
            return;
        };
        let drive = session.drive.clone();
        match self.cursor.clone() {
            Some(cursor) => spawn_keyed(
                CHANGES,
                async move { drive.changes(&cursor).await },
                DriveState::changes_polled,
            ),
            None => spawn_keyed(
                CHANGES,
                async move { drive.start_cursor().await },
                DriveState::cursor_started,
            ),
        }
    }

    fn cursor_started(&mut self, cursor: google_drive::Result<String>) {
        match cursor {
            Ok(cursor) => {
                self.cursor = Some(cursor);
                skip_view();
            }
            Err(error) => self.poll_failed(&error),
        }
    }

    fn changes_polled(&mut self, changes: google_drive::Result<Changes>) {
        match changes {
            Ok(changes) => {
                self.cursor = Some(changes.cursor);
                match changes.changes.is_empty() || self.renaming.is_some() {
                    true => skip_view(),
                    false => self.load(true),
                }
            }
            Err(error) => self.poll_failed(&error),
        }
    }

    fn poll_failed(&mut self, error: &Error) {
        tracing::debug!("failed to check Drive for changes: {error}");
        match error.is_signed_out() {
            true => self.signed_out.set(true),
            false => skip_view(),
        }
    }

    fn fetch_thumbnails(&mut self) {
        let Some(session) = &self.session else {
            return;
        };
        if *self.mode != ViewMode::Icons {
            return;
        }
        let known = self.thumbnails.borrow();
        let files: Vec<File> = self
            .rows
            .get(self.visible.start.min(self.rows.len())..self.visible.end.min(self.rows.len()))
            .unwrap_or_default()
            .iter()
            .map(|row| &row.file)
            .filter(|file| file.has_thumbnail && file.thumbnail_link.is_some())
            .filter(|file| !known.contains_key(&file.id) && !self.wanted.contains(&file.id))
            .cloned()
            .collect();
        drop(known);
        if files.is_empty() {
            return;
        }
        self.wanted.extend(files.iter().map(|file| file.id.clone()));
        let drive = session.drive.clone();
        let shelf = session.shelf.clone();
        spawn(
            async move {
                stream::iter(files)
                    .map(|file| {
                        let (drive, shelf) = (&drive, &shelf);
                        async move {
                            if let Some(kept) = shelf.thumbnail(&file) {
                                return (file.id, Some(kept));
                            }
                            let kept = drive
                                .thumbnail(&file, THUMBNAIL)
                                .await
                                .and_then(|picture| shelf.keep_thumbnail(&file, &picture));
                            match kept {
                                Ok(kept) => (file.id, kept),
                                Err(error) => {
                                    tracing::debug!("failed to fetch a Drive thumbnail: {error}");
                                    (file.id, None)
                                }
                            }
                        }
                    })
                    .buffer_unordered(AT_ONCE)
                    .collect::<Vec<_>>()
                    .await
            },
            |drive: &mut DriveState, pictured: Vec<(String, Option<Thumbnail>)>| {
                let mut thumbnails = drive.thumbnails.borrow_mut();
                for (id, thumbnail) in pictured {
                    match thumbnail {
                        Some(thumbnail) => {
                            thumbnails.insert(id, thumbnail);
                        }
                        None => {
                            drive.wanted.insert(id);
                        }
                    }
                }
                drop(thumbnails);
                drive.drawing.repaint();
                skip_view();
            },
        );
    }
}

async fn every(
    actions: impl Iterator<Item = impl Future<Output = google_drive::Result<()>>>,
) -> google_drive::Result<()> {
    let done: Vec<google_drive::Result<()>> = stream::iter(actions)
        .buffer_unordered(AT_ONCE)
        .collect()
        .await;
    done.into_iter().collect()
}
