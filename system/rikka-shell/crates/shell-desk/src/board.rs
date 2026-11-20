use std::cell::RefCell;
use std::rc::Rc;

use desk_layout::{Desks, Layout, WidgetId};
use futures_util::Stream;
use rikka_ui::{Observed, Observers};
use rikka_worker::Observable;
use shell_kit::StateFile;

const FILE: &str = "desk.toml";

const UNREADABLE: &str = "unreadable";

const SKIPPED: &str = "skipped";

#[derive(Clone)]
pub struct Board(Rc<Inner>);

struct Inner {
    desks: RefCell<Desks>,
    showing: RefCell<Option<String>>,
    observers: Observers,
    revision: Observable<u64>,
    file: Option<StateFile>,
}

impl Board {
    #[must_use]
    pub(crate) fn load() -> Board {
        let (desks, file) = match StateFile::config(FILE) {
            Some(file) => read(file),
            None => (Desks::default(), None),
        };
        Board(Rc::new(Inner {
            desks: RefCell::new(desks),
            showing: RefCell::new(None),
            observers: Observers::new(),
            revision: Observable::new(0),
            file,
        }))
    }

    #[must_use]
    pub fn layout(&self, wallpaper: Option<&str>) -> Layout {
        self.0.observers.read();
        self.0
            .desks
            .borrow()
            .layout(wallpaper)
            .cloned()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn predicted<R>(&self, wallpaper: Option<&str>, read: impl FnOnce(&Layout) -> R) -> R {
        self.0.observers.read();
        let desks = self.0.desks.borrow();
        let showing = self.0.showing.borrow();
        match desks
            .layout(wallpaper)
            .or_else(|| desks.layout(showing.as_deref()))
        {
            Some(layout) => read(layout),
            None => read(&Layout::default()),
        }
    }

    #[must_use]
    pub fn with_shown<R>(&self, read: impl FnOnce(&Layout) -> R) -> R {
        self.0.observers.read();
        let desks = self.0.desks.borrow();
        match desks.layout(self.0.showing.borrow().as_deref()) {
            Some(layout) => read(layout),
            None => read(&Layout::default()),
        }
    }

    #[must_use]
    pub fn showing(&self) -> Option<String> {
        self.0.observers.read();
        self.0.showing.borrow().clone()
    }

    #[must_use]
    pub fn shown_layout(&self) -> Layout {
        let showing = self.showing();
        self.layout(showing.as_deref())
    }

    pub(crate) fn show(&self, wallpaper: Option<String>) {
        if *self.0.showing.borrow() == wallpaper {
            return;
        }
        let before = self.0.showing.replace(wallpaper.clone());
        let arrived = self
            .0
            .desks
            .borrow_mut()
            .arrive(wallpaper.as_deref(), before.as_deref());
        if arrived {
            self.keep();
        }
        self.announce();
    }

    pub fn change(&self, wallpaper: Option<&str>, edit: impl FnOnce(&mut Layout)) -> bool {
        let changed = {
            let mut desks = self.0.desks.borrow_mut();
            let layout = desks.layout_mut(wallpaper);
            let before = layout.clone();
            edit(layout);
            *layout != before
        };
        if changed {
            self.keep();
            self.announce();
        }
        changed
    }

    #[must_use]
    pub fn fresh_id(&self) -> WidgetId {
        self.0.desks.borrow().fresh_id()
    }

    pub fn changes(&self) -> impl Stream<Item = u64> + Send + 'static {
        self.0.revision.changes()
    }

    fn announce(&self) {
        self.0.revision.modify(|revision| {
            *revision += 1;
            true
        });
        self.0.observers.changed();
    }

    fn keep(&self) {
        let Some(file) = &self.0.file else {
            return;
        };
        file.save(self.0.desks.borrow().document());
    }
}

impl Observed for Board {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}

impl std::fmt::Debug for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Board")
            .field("showing", &self.0.showing.borrow())
            .finish_non_exhaustive()
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Some(file) = &self.file {
            file.flush();
        }
    }
}

fn read(file: StateFile) -> (Desks, Option<StateFile>) {
    let text = match file.read() {
        Ok(Some(text)) => text,
        Ok(None) => return (Desks::default(), Some(file)),
        Err(error) => {
            tracing::error!(
                "failed to read `{}`, widget changes will not be saved: {error}",
                file.path().display()
            );
            return (Desks::default(), None);
        }
    };
    match Desks::parse(&text) {
        Ok(parsed) if parsed.skipped.is_empty() => (parsed.desks, Some(file)),
        Ok(parsed) => {
            for skipped in &parsed.skipped {
                tracing::warn!(
                    "skipped widget on {}: {}",
                    skipped.wallpaper.as_deref().unwrap_or("the bare desk"),
                    skipped.reason
                );
            }
            match file.set_aside(SKIPPED, &text) {
                Ok(aside) => {
                    tracing::warn!("saved previous desk layout to `{}`", aside.display());
                    (parsed.desks, Some(file))
                }
                Err(error) => {
                    tracing::error!("not saving widgets, to preserve skipped entries: {error}");
                    (parsed.desks, None)
                }
            }
        }
        Err(unreadable) => match file.set_aside(UNREADABLE, &text) {
            Ok(aside) => {
                tracing::error!("{unreadable}, backed up to `{}`", aside.display());
                (Desks::default(), Some(file))
            }
            Err(error) => {
                tracing::error!("{unreadable}, failed to back it up: {error}");
                (Desks::default(), None)
            }
        },
    }
}
