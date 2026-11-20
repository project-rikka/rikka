use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use futures_util::Stream;
use futures_util::stream::BoxStream;
use rikka_ui::ImageSource;
use rikka_worker::Observable;
use shell_kit::taskbar::Shape;
use shell_wallpaper::{self as wallpaper, Media};

#[derive(Clone, Debug, Default)]
pub(crate) struct Desktop(Rc<Inner>);

#[derive(Debug, Default)]
struct Inner {
    wallpaper: RefCell<Option<PathBuf>>,
    showing: RefCell<Option<Showing>>,
    reading: Cell<bool>,
    refused: RefCell<Option<PathBuf>>,
    taskbar: Cell<Option<Shape>>,
    moved: Observable<()>,
}

#[derive(Debug)]
struct Showing {
    path: PathBuf,
    media: Media,
}

impl Desktop {
    pub(super) fn set_wallpaper(&self, wallpaper: Option<PathBuf>) {
        if self
            .0
            .showing
            .borrow()
            .as_ref()
            .map(|showing| &showing.path)
            != wallpaper.as_ref()
        {
            self.0.showing.borrow_mut().take();
        }
        *self.0.wallpaper.borrow_mut() = wallpaper;
        self.0.moved.set(());
    }

    pub(crate) fn set_taskbar(&self, taskbar: Option<Shape>) {
        if self.0.taskbar.replace(taskbar) != taskbar {
            self.0.moved.set(());
        }
    }

    pub(super) fn changes(&self) -> impl Stream<Item = ()> + Send + 'static + use<> {
        self.0.moved.changes()
    }

    #[must_use]
    pub(super) fn unready(&self) -> Option<BoxStream<'static, ()>> {
        let showing = self.0.showing.borrow();
        let media = &showing.as_ref()?.media;
        (!media.is_ready()).then(|| media.changes())
    }

    pub(super) fn load_shared(&self, cover: Option<(u32, u32)>) {
        if self.0.showing.borrow().is_some() {
            return;
        }
        let Some(path) = self.0.wallpaper.borrow().clone() else {
            return;
        };
        if let Some(media) = wallpaper::shared(&path, cover) {
            *self.0.showing.borrow_mut() = Some(Showing { path, media });
        }
    }

    #[must_use]
    pub(crate) fn wanting(&self) -> Option<PathBuf> {
        if self.0.reading.get() || self.0.showing.borrow().is_some() {
            return None;
        }
        let wallpaper = self.0.wallpaper.borrow().clone()?;
        if self.0.refused.borrow().as_ref() == Some(&wallpaper) {
            return None;
        }
        self.0.reading.set(true);
        Some(wallpaper)
    }

    pub(crate) fn finish_loading(&self, path: PathBuf, read: Result<Media, String>) {
        self.0.reading.set(false);
        if self.0.wallpaper.borrow().as_ref() != Some(&path) {
            return;
        }
        match read {
            Ok(media) => *self.0.showing.borrow_mut() = Some(Showing { path, media }),
            Err(reason) => {
                tracing::warn!("overview wallpaper unavailable: {reason}");
                *self.0.refused.borrow_mut() = Some(path);
            }
        }
        self.0.moved.set(());
    }

    #[must_use]
    pub(crate) fn wallpaper(&self) -> Option<ImageSource> {
        let current = self.0.showing.borrow();
        let media = &current.as_ref()?.media;
        media.is_ready().then(|| media.picture().source())
    }

    #[must_use]
    pub(super) fn current_wallpaper(&self) -> Option<(PathBuf, bool)> {
        let current = self.0.showing.borrow();
        let showing = current.as_ref()?;
        Some((showing.path.clone(), showing.media.is_ready()))
    }

    #[must_use]
    pub(crate) fn taskbar(&self) -> Option<Shape> {
        self.0.taskbar.get()
    }
}
