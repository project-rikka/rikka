mod backdrop;
mod cell;
mod layout;
mod listing;
mod menu;
mod sheet;

use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::{StreamExt as _, stream};
use proton_config::Config;
use proton_files::{
    Direction, DriveCache, Entry, Listed, Mounted, Pictured, SortKey, ThumbRequest, Thumbs,
    Transfers, Tree, listing, mount, open_viewer, pictures, preview, rename, upload,
};
use proton_session::{ErrorBanner, Failure};
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse};
use rikka_proton_api::offload;
use rikka_proton_cache::Cache;
use rikka_proton_drive::{DriveClient, File, Folder, LinkId, ShareKeys, VolumeId};
use rikka_ui::Place;
use rikka_widgets::column;

use crate::clients::Clients;
use crate::pane;
use crate::workspace::{ProductState, Workspace, WorkspaceId};

const READING: &str = "drive.children";
const THUMBING: &str = "drive.thumbs";
const THUMBS_PER_WAVE: usize = 30;
const WAVES_AT_ONCE: usize = 2;

#[derive(Debug)]
pub(crate) enum Sheet {
    Naming(TextDocument),
    Renaming(LinkId, TextDocument),
}

#[derive(Debug, State)]
pub(crate) struct Drive {
    clients: Clients,
    records: DriveCache,
    workspace_id: WorkspaceId,
    tree: Tree,
    thumbs: Thumbs,
    asking: VecDeque<ThumbRequest>,
    thumbing: Option<LinkId>,
    waves: [bool; WAVES_AT_ONCE],
    transfers: Transfers,
    viewing: HashMap<LinkId, Viewing>,
    selected: Option<LinkId>,
    visible: Range<usize>,
    loading: bool,
    started: bool,
    tiles: bool,
    error: ErrorBanner,
    sheet: Option<Sheet>,
    menu: Option<Point>,
    place: Place,
    widths: ColumnWidths,
}

impl Drive {
    pub(crate) fn describe(&mut self, clients: Clients) {
        self.clients = clients;
    }

    pub(crate) fn new(
        clients: Clients,
        cache: Cache,
        workspace_id: WorkspaceId,
        config: &Config,
    ) -> Drive {
        Drive {
            clients,
            thumbs: Thumbs::new(),
            records: DriveCache::new(cache),
            workspace_id,
            tree: Tree::new(),
            asking: VecDeque::new(),
            thumbing: None,
            waves: [false; WAVES_AT_ONCE],
            transfers: Transfers::new(),
            viewing: HashMap::new(),
            selected: None,
            visible: 0..0,
            loading: false,
            started: false,
            tiles: config.drive_tiles(),
            error: ErrorBanner::default(),
            sheet: None,
            menu: None,
            place: Place::default(),
            widths: ColumnWidths::new(),
        }
    }

    pub(crate) fn opened(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        self.recall_root();
        self.mount();
    }

    pub(crate) fn tree(&self) -> &Tree {
        &self.tree
    }

    pub(crate) fn thumbs(&self) -> &Thumbs {
        &self.thumbs
    }

    pub(crate) fn transfers(&self) -> &Transfers {
        &self.transfers
    }

    pub(crate) fn selected(&self) -> Option<usize> {
        self.tree.place(self.selected.as_ref()?)
    }

    pub(crate) fn is_busy(&self) -> bool {
        self.loading
    }

    pub(crate) fn shows_tiles(&self) -> bool {
        self.tiles
    }

    pub(crate) fn place(&self) -> &Place {
        &self.place
    }

    pub(crate) fn widths(&self) -> &ColumnWidths {
        &self.widths
    }

    pub(crate) fn sheet(&self) -> Option<&Sheet> {
        self.sheet.as_ref()
    }

    pub(crate) fn menu(&self) -> Option<Point> {
        self.menu
    }

    pub(crate) fn tile(&mut self, tiles: bool) {
        self.tiles = tiles;
    }

    pub(crate) fn sort(&mut self, sorted: SortKey) {
        self.tree.sort(sorted);
    }

    pub(crate) fn select(&mut self, at: usize) {
        self.pick(Some(at));
    }

    pub(crate) fn pick(&mut self, at: Option<usize>) {
        let selected = at
            .and_then(|at| self.tree.entry(at))
            .map(|entry| entry.id.clone());
        if self.selected == selected {
            skip_view();
            return;
        }
        self.selected = selected;
    }

    pub(crate) fn open_menu(&mut self, at: Point) {
        self.menu = Some(at);
    }

    pub(crate) fn open_menu_aside(&mut self, at: Point) {
        self.selected = None;
        self.menu = Some(at);
    }

    pub(crate) fn close_menu(&mut self) {
        self.menu = None;
    }

    pub(crate) fn showing(&mut self, visible: Range<usize>) {
        if self.visible != visible {
            self.visible = visible;
            self.want_thumbs();
        }
        skip_view();
    }

    pub(crate) fn enter(&mut self, at: usize) {
        let Some(entry) = self.tree.entry(at).cloned() else {
            return;
        };
        if !entry.folder {
            self.view(&entry);
            return;
        }
        if self.tree.enter(&entry.id) {
            self.selected = None;
            self.arrived();
        }
    }

    pub(crate) fn open_selected(&mut self) {
        let Some(at) = self.selected() else {
            return;
        };
        self.enter(at);
    }

    pub(crate) fn back_to(&mut self, depth: usize) {
        if self.tree.back_to(depth) {
            self.selected = None;
            self.arrived();
        }
    }

    pub(crate) fn up(&mut self) {
        self.back_to(self.tree.depth().saturating_sub(1));
    }

    pub(crate) fn refresh(&mut self) {
        match self.tree.is_open() {
            true => self.read(),
            false => self.mount(),
        }
    }

    pub(crate) fn start_folder(&mut self) {
        self.sheet = Some(Sheet::Naming(TextDocument::new("")));
    }

    pub(crate) fn close_sheet(&mut self) {
        self.sheet = None;
    }

    pub(crate) fn make_folder(&mut self) {
        let (Some(Sheet::Naming(name)), Some(share), Some(folder)) = (
            self.sheet.as_ref(),
            self.tree.share().cloned(),
            self.tree.folder().cloned(),
        ) else {
            return;
        };
        let name = name.text().trim().to_owned();
        if name.is_empty() {
            return;
        }

        self.sheet = None;
        self.loading = true;
        let client = self.clients.drive();
        self.spawn(
            async move {
                client
                    .create_folder(&share, &folder, &name)
                    .await
                    .map(drop)
                    .map_err(Failure::from)
            },
            |drive: &mut Drive, done: Result<(), Failure>| drive.acted(done),
        );
    }

    pub(crate) fn start_rename(&mut self) {
        let Some(entry) = self.chosen() else {
            return;
        };
        self.sheet = Some(Sheet::Renaming(
            entry.id.clone(),
            TextDocument::new(&entry.name),
        ));
    }

    pub(crate) fn rename(&mut self) {
        let (Some(Sheet::Renaming(id, document)), Some(share), Some(folder)) = (
            self.sheet.as_ref(),
            self.tree.share().cloned(),
            self.tree.folder().cloned(),
        ) else {
            return;
        };
        let name = document.text().trim().to_owned();
        let id = id.clone();
        if name.is_empty() {
            return;
        }

        self.sheet = None;
        self.loading = true;
        let client = self.clients.drive();
        self.spawn(
            async move { Ok(rename(&client, &share, &folder, &id, &name).await?) },
            |drive: &mut Drive, done: Result<(), Failure>| drive.acted(done),
        );
    }

    pub(crate) fn throw(&mut self) {
        let (Some(entry), Some(volume)) = (self.chosen().cloned(), self.tree.volume_id().cloned())
        else {
            return;
        };

        self.tree.remove(std::slice::from_ref(&entry.id));
        self.selected = None;
        self.loading = true;
        let client = self.clients.drive();
        let ids = vec![entry.id];
        self.spawn(
            async move { Ok(client.trash(&volume, &ids).await.map(drop)?) },
            |drive: &mut Drive, done: Result<(), Failure>| drive.acted(done),
        );
    }

    pub(crate) fn erase(&mut self) {
        let (Some(entry), Some(volume)) = (self.chosen().cloned(), self.tree.volume_id().cloned())
        else {
            return;
        };

        self.tree.remove(std::slice::from_ref(&entry.id));
        self.selected = None;
        self.loading = true;
        let client = self.clients.drive();
        let ids = vec![entry.id];
        self.spawn(
            async move { Ok(client.delete(&volume, &ids).await.map(drop)?) },
            |drive: &mut Drive, done: Result<(), Failure>| drive.acted(done),
        );
    }

    pub(crate) fn upload(&mut self) {
        let (Some(share), Some(folder)) = (self.tree.share().cloned(), self.tree.folder().cloned())
        else {
            return;
        };

        let parent = window_identifier();
        self.spawn_blocking(
            move || {
                Chooser::open(tr!("proton.drive.upload").as_str())
                    .parent(parent)
                    .multiple(true)
                    .run()
                    .map(paths)
                    .unwrap_or_default()
            },
            move |drive: &mut Drive, chosen: Vec<std::path::PathBuf>| {
                for path in chosen {
                    drive.put(&share, &folder, path);
                }
            },
        );
    }

    pub(crate) fn download_selected(&mut self) {
        let Some(entry) = self.chosen().cloned() else {
            return;
        };
        self.download(&entry);
    }

    pub(crate) fn forget(&mut self, id: u64) {
        self.transfers.forget(id);
    }

    fn chosen(&self) -> Option<&Entry> {
        let id = self.selected.as_ref()?;
        self.tree.entries().iter().find(|entry| &entry.id == id)
    }

    fn put(&mut self, share: &ShareKeys, folder: &Folder, path: std::path::PathBuf) {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let id = self.transfers.start(&name, Direction::Up, 0);
        let client = self.clients.drive();
        let (share, folder) = (share.clone(), folder.clone());

        self.spawn_blocking(
            move || {
                let picture = preview(&path);
                (path, picture)
            },
            move |drive: &mut Drive, (path, picture): (std::path::PathBuf, Option<Vec<u8>>)| {
                let (client, share, folder) = (client.clone(), share.clone(), folder.clone());
                drive.spawn(
                    async move {
                        upload(&client, &share, &folder, &path, picture)
                            .await
                            .map(drop)
                            .map_err(Failure::from)
                    },
                    move |drive: &mut Drive, done: Result<(), Failure>| match done {
                        Ok(()) => {
                            drive.transfers.finish(id);
                            drive.read();
                        }
                        Err(reason) => drive.transfers.failed(id, reason.to_string()),
                    },
                );
                skip_view();
            },
        );
    }

    fn download(&mut self, entry: &Entry) {
        self.fetch(entry, Fetch::Keep);
    }

    fn view(&mut self, entry: &Entry) {
        match self.viewing.get(&entry.id) {
            Some(Viewing::Fetching) => skip_view(),
            Some(Viewing::Cached { modified, path })
                if *modified == entry.modified && path.exists() =>
            {
                let path = path.clone();
                self.show(&path);
            }
            _ => self.fetch(entry, Fetch::Show),
        }
    }

    fn fetch(&mut self, entry: &Entry, fetch: Fetch) {
        let (Some(volume), Some(folder)) =
            (self.tree.volume_id().cloned(), self.tree.folder().cloned())
        else {
            return;
        };
        let known = self.tree.file(&entry.id).cloned();
        let total = u64::try_from(entry.size).unwrap_or_default();
        let id = self.transfers.start(&entry.name, fetch.direction(), total);
        let shown = matches!(fetch, Fetch::Show).then(|| Shown {
            link: entry.id.clone(),
            modified: entry.modified,
        });
        if let Some(shown) = &shown {
            self.viewing.insert(shown.link.clone(), Viewing::Fetching);
        }

        let client = self.clients.drive();
        let link_id = entry.id.clone();
        let (sender, steps) = futures_channel::mpsc::unbounded::<Step>();
        let reporting = sender.clone();

        self.spawn(
            async move {
                let moved = move |done| drop(reporting.unbounded_send(Step::Moved(done)));
                let step =
                    match fetched(&client, &volume, &folder, &link_id, known, fetch, moved).await {
                        Ok(path) => Step::Done(path),
                        Err(error) => Step::Failed(error.to_string()),
                    };
                drop(sender.unbounded_send(step));
            },
            |_drive: &mut Drive, (): ()| skip_view(),
        );
        self.stream(steps, move |drive: &mut Drive, step: Step| {
            drive.stepped(id, shown.as_ref(), step);
        });
    }

    fn stepped(&mut self, id: u64, shown: Option<&Shown>, step: Step) {
        match (step, shown) {
            (Step::Moved(done), _) => self.transfers.progress(id, done),
            (Step::Done(_), None) => self.transfers.finish(id),
            (Step::Done(path), Some(shown)) => {
                self.transfers.forget(id);
                self.show(&path);
                self.viewing.insert(
                    shown.link.clone(),
                    Viewing::Cached {
                        modified: shown.modified,
                        path,
                    },
                );
            }
            (Step::Failed(reason), shown) => {
                if let Some(shown) = shown {
                    self.viewing.remove(&shown.link);
                }
                self.transfers.failed(id, reason);
            }
        }
    }

    fn show(&mut self, path: &Path) {
        if let Err(error) = proton_files::show(path) {
            self.error.set(error);
        }
    }

    fn want_thumbs(&mut self) {
        let entries = Arc::clone(self.tree.entries());
        let visible = self.visible.start.min(entries.len())..self.visible.end.min(entries.len());
        let asking = self.thumbs.request(entries[visible].iter());

        for ask in asking.into_iter().rev() {
            self.asking.push_front(ask);
        }
        self.fetch_thumbs();
    }

    fn fetch_thumbs(&mut self) {
        let Some(at) = self.thumbing.clone() else {
            return;
        };
        while let Some(slot) = self.waves.iter().position(|busy| !busy)
            && !self.asking.is_empty()
        {
            let count = THUMBS_PER_WAVE.min(self.asking.len());
            let wave: Vec<(ThumbRequest, Option<File>)> = self
                .asking
                .drain(..count)
                .map(|ask| {
                    let file = self.tree.file(ask.id()).cloned();
                    (ask, file)
                })
                .collect();

            self.waves[slot] = true;
            let pictured = pictures(&self.clients.drive(), self.tree.share(), wave)
                .map(Some)
                .chain(stream::once(std::future::ready(None)));
            let wave_of = at.clone();
            self.stream_keyed(
                (THUMBING, at.clone(), slot),
                pictured,
                move |drive: &mut Drive, pictured: Option<Pictured>| match pictured {
                    Some(pictured) => drive.thumb_arrived(pictured),
                    None => drive.wave_ended(&wave_of, slot),
                },
            );
        }
    }

    fn halt_thumbs(&mut self) {
        if let Some(at) = self.thumbing.take() {
            for (slot, busy) in self.waves.iter().enumerate() {
                if *busy {
                    cancel((THUMBING, at.clone(), slot));
                }
            }
        }
        self.waves = [false; WAVES_AT_ONCE];
    }

    fn thumb_arrived(&mut self, (ask, picture): Pictured) {
        let cached = match picture {
            Ok(thumb) => self.thumbs.insert(&ask, thumb),
            Err(_) => {
                self.thumbs.cancel(&ask);
                false
            }
        };
        if !cached {
            skip_view();
        }
    }

    fn wave_ended(&mut self, at: &LinkId, slot: usize) {
        if self.thumbing.as_ref() == Some(at) {
            self.waves[slot] = false;
            self.fetch_thumbs();
        }
        skip_view();
    }

    fn recall_root(&mut self) {
        let records = self.records.clone();
        self.spawn_blocking(
            move || records.cached_root_listing(),
            |drive: &mut Drive, recalled: Option<(LinkId, Vec<Entry>)>| {
                let Some((root, entries)) = recalled else {
                    skip_view();
                    return;
                };
                match drive.tree.load_cached_root(root, root_name(), entries) {
                    true => drive.shown(),
                    false => skip_view(),
                }
            },
        );
    }

    fn mount(&mut self) {
        self.loading = true;
        let client = self.clients.drive();
        let records = self.records.clone();
        let order = self.tree.order();

        self.spawn(
            async move { mount(client, records, order).await.map_err(Failure::from) },
            |drive: &mut Drive, mounted: Result<Mounted, Failure>| drive.mounted(mounted),
        );
    }

    fn mounted(&mut self, mounted: Result<Mounted, Failure>) {
        let Mounted {
            share,
            root,
            listed,
        } = match mounted {
            Ok(mounted) => mounted,
            Err(reason) => {
                self.loading = false;
                self.error.set(reason);
                return;
            }
        };
        let at = root.id().clone();
        self.tree.opened(share, root, root_name());

        match listed {
            Some(page) => {
                self.loading = false;
                self.tree.listed(&at, page);
                self.shown();
            }
            None => {
                self.shown();
                self.read();
            }
        }
    }

    fn arrived(&mut self) {
        if !self.tree.at().is_some_and(|at| self.tree.is_cached(at)) {
            self.recall_folder();
        }
        self.shown();
        self.read();
    }

    fn recall_folder(&mut self) {
        let Some(at) = self.tree.at().cloned() else {
            return;
        };
        let records = self.records.clone();
        let location = at.clone();

        self.spawn_blocking(
            move || records.cached_entries(&location),
            move |drive: &mut Drive, cached: Option<Vec<Entry>>| {
                let shown = cached.is_some_and(|entries| drive.tree.load_cached(&at, entries));
                match shown {
                    true => drive.shown(),
                    false => skip_view(),
                }
            },
        );
    }

    fn read(&mut self) {
        let (Some(volume), Some(folder)) =
            (self.tree.volume_id().cloned(), self.tree.folder().cloned())
        else {
            return;
        };

        self.loading = true;
        let at = folder.id().clone();
        self.tree.begin(&at);
        let pages = listing(
            &self.clients.drive(),
            &volume,
            &folder,
            self.records.clone(),
            self.tree.order(),
        );
        self.stream_keyed(READING, pages, move |drive: &mut Drive, listed: Listed| {
            drive.listed(&at, listed);
        });
    }

    fn listed(&mut self, at: &LinkId, listed: Listed) {
        match listed {
            Listed::Page(page) => match self.tree.append_page(at, page) {
                true => self.shown(),
                false => skip_view(),
            },
            Listed::Done => {
                self.loading = false;
                self.tree.finish_listing(at);
                self.shown();
            }
            Listed::Failed(reason) => {
                self.loading = false;
                self.error.set(Failure::from(reason));
            }
        }
    }

    fn shown(&mut self) {
        if self.thumbing.as_ref() != self.tree.at() {
            self.halt_thumbs();
            self.thumbing = self.tree.at().cloned();
        }
        let entries = Arc::clone(self.tree.entries());
        let present: HashSet<&LinkId> = entries.iter().map(|entry| &entry.id).collect();
        self.asking.retain(|ask| present.contains(ask.id()));
        self.thumbs.retain(&entries);
        self.want_thumbs();
    }

    fn acted(&mut self, done: Result<(), Failure>) {
        self.loading = false;
        match done {
            Ok(()) => self.read(),
            Err(reason) => self.error.set(reason),
        }
    }
}

#[derive(Debug)]
struct Shown {
    link: LinkId,
    modified: jiff::Timestamp,
}

#[derive(Debug)]
enum Viewing {
    Fetching,
    Cached {
        modified: jiff::Timestamp,
        path: PathBuf,
    },
}

#[derive(Clone, Copy)]
enum Fetch {
    Keep,
    Show,
}

impl Fetch {
    fn direction(self) -> Direction {
        match self {
            Fetch::Keep => Direction::Down,
            Fetch::Show => Direction::View,
        }
    }
}

async fn fetched(
    client: &DriveClient,
    volume: &VolumeId,
    folder: &Folder,
    link: &LinkId,
    known: Option<File>,
    fetch: Fetch,
    moved: impl FnMut(u64) + Send,
) -> Result<PathBuf, proton_files::Error> {
    let file = match known {
        Some(file) => file,
        None => client.file(volume, link, folder).await?,
    };
    match fetch {
        Fetch::Keep => {
            let into = offload(proton_files::downloads).await;
            Ok(client
                .download(volume, &file)
                .save_into(&into, moved)
                .await?)
        }
        Fetch::Show => open_viewer(client, volume, &file, moved).await,
    }
}

#[derive(Debug)]
enum Step {
    Moved(u64),
    Done(PathBuf),
    Failed(String),
}

fn paths(chosen: ChooserResponse) -> Vec<std::path::PathBuf> {
    match chosen {
        ChooserResponse::Files(paths) => paths,
        ChooserResponse::Cancelled => Vec::new(),
    }
}

fn root_name() -> String {
    tr!("proton.drive.root").as_str().to_owned()
}

pub(crate) fn view(drive: &Drive) -> impl View {
    sheet::stage(
        column! {
            listing::toolbar(drive);
            rule();
            pane::error_banner(&drive.error, |drive: &mut Drive| &mut drive.error);
            listing::view(drive).grow(1.0);
            listing::transfers(drive);
            if let Some(shown) = drive.sheet() {
                sheet::view(shown);
            }
            if let Some(at) = drive.menu() {
                menu::view(drive, at);
            }
        }
        .fill(),
    )
}

impl ProductState for Drive {
    fn from_workspace(workspace: &mut Workspace) -> &mut Drive {
        &mut workspace.drive
    }

    fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
}
