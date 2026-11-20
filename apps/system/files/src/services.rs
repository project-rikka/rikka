use rikka_sync::{Mutex, MutexGuard};
use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;

use files_archive::ArchiveBackend;
use files_core::{FolderKind, Location};
use files_drives::{Drives, Volume, VolumeId};
use files_open::Applications;
use files_places::Sidebar;
use files_thumbs::Thumbnails;
use files_undo::History;
use files_vfs::Vfs;
use rikka_app::Sender;
use rikka_mime::Database;
use rikka_thumbnails::Cache;
use rikka_ui::{Observers, State};

use crate::places;
use crate::programs::Programs;

thread_local! {
    static SHARED: OnceCell<Rc<SharedServices>> = const { OnceCell::new() };
}

pub(crate) struct Services {
    shared: Rc<SharedServices>,
    window: WindowKey,
}

impl Services {
    pub(crate) fn attach<S: State, T: 'static, D: 'static>(
        sender: &Sender<S>,
        thumbnails: impl Fn(&mut T, Vec<Location>) + Send + Sync + 'static,
        disks_changed: impl Fn(&mut D) + Send + Sync + 'static,
        mime: impl FnOnce() -> Arc<Database>,
    ) -> Services {
        let shared =
            SHARED.with(|shared| Rc::clone(shared.get_or_init(|| SharedServices::open(mime()))));
        let window = shared.next_key();
        let thumbnail_sender = sender.clone();
        let disks_sender = sender.clone();
        let thumbnails = Arc::new(thumbnails);
        let arrivals = Arc::new(Arrivals::default());
        let disks_changed = Arc::new(disks_changed);
        shared.listeners.add(Listener {
            window,
            thumbnail: Box::new(move |at| {
                if !arrivals.push(at) {
                    return;
                }
                let (arrived, arrivals) = (Arc::clone(&thumbnails), Arc::clone(&arrivals));
                thumbnail_sender
                    .send_to(move |target: &mut T| arrived(target, arrivals.take()))
                    .ok();
            }),
            disks_changed: Box::new(move || {
                let changed = Arc::clone(&disks_changed);
                disks_sender
                    .send_to(move |target: &mut D| changed(target))
                    .ok();
            }),
        });
        Services { shared, window }
    }

    pub(crate) fn shared(&self) -> Rc<SharedServices> {
        Rc::clone(&self.shared)
    }

    pub(crate) fn showing(&self, visible: HashSet<Location>) {
        self.shared
            .visible
            .borrow_mut()
            .insert(self.window, visible);
        self.shared.retain_thumbnails();
    }
}

impl Deref for Services {
    type Target = SharedServices;

    fn deref(&self) -> &SharedServices {
        &self.shared
    }
}

impl Drop for Services {
    fn drop(&mut self) {
        self.shared.listeners.remove(self.window);
        self.shared.visible.borrow_mut().remove(&self.window);
        self.shared.retain_thumbnails();
    }
}

impl std::fmt::Debug for Services {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Services")
            .field("window", &self.window)
            .field("shared", &self.shared)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct WindowKey(u64);

pub(crate) struct SharedServices {
    pub(crate) vfs: Arc<Vfs>,
    pub(crate) mime: Arc<Database>,
    pub(crate) thumbnails: Option<Thumbnails>,
    programs: OnceCell<Programs>,
    pub(crate) drives: Arc<Drives>,
    places: RefCell<Places>,
    shown: Observers,
    visible: RefCell<HashMap<WindowKey, HashSet<Location>>>,
    listeners: Arc<Listeners>,
    next: Cell<u64>,
    pub(crate) history: RefCell<History>,
}

impl SharedServices {
    fn open(mime: Arc<Database>) -> Rc<SharedServices> {
        let mut registry = Vfs::new(Arc::clone(&mime));
        registry.register(Arc::new(ArchiveBackend::new(Arc::clone(&mime))));

        let listeners = Arc::new(Listeners::default());
        let thumbnails = Cache::shared().map(|cache| {
            let thumbnail_sender = Arc::clone(&listeners);
            Thumbnails::new(
                cache,
                Arc::new(move |at: Location| thumbnail_sender.thumbnail(at)),
            )
        });

        let disks_sender = Arc::clone(&listeners);
        let drives = Arc::new(Drives::open(move || disks_sender.disks_changed()));

        Rc::new(SharedServices {
            vfs: Arc::new(registry),
            mime,
            thumbnails,
            programs: OnceCell::new(),
            drives,
            places: RefCell::new(Places::default()),
            shown: Observers::new(),
            visible: RefCell::new(HashMap::new()),
            listeners,
            next: Cell::new(0),
            history: RefCell::new(History::default()),
        })
    }

    pub(crate) fn preload_applications(&self) {
        self.programs.get_or_init(Programs::start);
    }

    pub(crate) fn applications(&self) -> Arc<Applications> {
        self.programs.get_or_init(Programs::start).applications()
    }

    pub(crate) fn rows(&self) -> Rc<Vec<places::Row>> {
        self.shown.read();
        Rc::clone(&self.places.borrow().rows)
    }

    pub(crate) fn row(&self, index: usize) -> Option<places::Row> {
        self.shown.read();
        self.places.borrow().rows.get(index).cloned()
    }

    pub(crate) fn is_pinned(&self, at: &Location) -> bool {
        self.shown.read();
        self.places.borrow().sidebar.is_pinned(at)
    }

    pub(crate) fn folder_kind(&self, at: &Location) -> Option<FolderKind> {
        self.places.borrow().sidebar.folder_kind(at)
    }

    pub(crate) fn volume(&self, id: &VolumeId) -> Option<Volume> {
        self.places
            .borrow()
            .volumes
            .iter()
            .find(|volume| volume.id == *id)
            .cloned()
    }

    pub(crate) fn reload_places(&self) {
        let mut places = self.places.borrow_mut();
        places.sidebar = Sidebar::load(&places.volumes);
        places.rebuild_rows();
        self.shown.changed();
    }

    pub(crate) fn working(&self, id: &VolumeId, running: bool) {
        let mut places = self.places.borrow_mut();
        if running {
            places.busy.insert(id.clone());
        } else {
            places.busy.remove(id);
        }
        places.rebuild_rows();
        self.shown.changed();
    }

    pub(crate) fn claim_automount(&self, arrived: Vec<VolumeId>) -> Vec<VolumeId> {
        let mut places = self.places.borrow_mut();
        arrived
            .into_iter()
            .filter(|id| places.automounted.insert(id.clone()))
            .collect()
    }

    pub(crate) fn adopt(&self, volumes: Vec<Volume>) {
        {
            let mut places = self.places.borrow_mut();
            if places.volumes == volumes {
                return;
            }
            places
                .busy
                .retain(|id| volumes.iter().any(|volume| volume.id == *id));
            places
                .automounted
                .retain(|id| volumes.iter().any(|volume| volume.id == *id));
            places.volumes = volumes;
        }
        self.reload_places();
    }

    fn next_key(&self) -> WindowKey {
        let key = self.next.get();
        self.next.set(key + 1);
        WindowKey(key)
    }

    fn retain_thumbnails(&self) {
        let Some(thumbnails) = &self.thumbnails else {
            return;
        };
        let visible = self.visible.borrow();
        let visible_locations: HashSet<Location> = visible.values().flatten().cloned().collect();
        thumbnails.retain(&visible_locations);
    }
}

impl std::fmt::Debug for SharedServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let places = self.places.borrow();
        f.debug_struct("SharedServices")
            .field("places", &places.rows.len())
            .field("volumes", &places.volumes.len())
            .field("windows", &self.visible.borrow().len())
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
struct Places {
    sidebar: Sidebar,
    rows: Rc<Vec<places::Row>>,
    volumes: Vec<Volume>,
    busy: HashSet<VolumeId>,
    automounted: HashSet<VolumeId>,
}

impl Places {
    fn rebuild_rows(&mut self) {
        self.rows = Rc::new(places::rows(&self.sidebar, &self.busy));
    }
}

#[derive(Default)]
struct Arrivals(Mutex<Vec<Location>>);

impl Arrivals {
    fn push(&self, at: Location) -> bool {
        let mut pending = self.lock();
        pending.push(at);
        pending.len() == 1
    }

    fn take(&self) -> Vec<Location> {
        std::mem::take(&mut *self.lock())
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Location>> {
        self.0.lock()
    }
}

#[derive(Default)]
struct Listeners(Mutex<Vec<Listener>>);

impl Listeners {
    fn add(&self, listener: Listener) {
        self.lock().push(listener);
    }

    fn remove(&self, window: WindowKey) {
        self.lock().retain(|listener| listener.window != window);
    }

    fn thumbnail(&self, at: Location) {
        for listener in self.lock().iter() {
            (listener.thumbnail)(at.clone());
        }
    }

    fn disks_changed(&self) {
        for listener in self.lock().iter() {
            (listener.disks_changed)();
        }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Listener>> {
        self.0.lock()
    }
}

struct Listener {
    window: WindowKey,
    thumbnail: Box<dyn Fn(Location) + Send>,
    disks_changed: Box<dyn Fn() + Send>,
}
