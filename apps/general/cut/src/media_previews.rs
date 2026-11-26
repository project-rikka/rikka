use std::path::Path;
use std::rc::Rc;

use cut_filmstrip::Filmstrips;
pub(crate) use cut_preferences::Display;
use cut_waveform::Waveforms;
use rikka_ui::Watchers;

#[derive(Clone)]
pub(crate) struct MediaPreviews(Rc<Inner>);

struct Inner {
    filmstrips: Option<Filmstrips>,
    waveforms: Option<Waveforms>,
    watchers: Watchers,
}

impl Default for MediaPreviews {
    fn default() -> MediaPreviews {
        let cache = rikka_dirs::Paths::resolve()
            .inspect_err(|error| tracing::debug!(%error, "waveform cache unavailable"))
            .ok()
            .map(|paths| paths.cache_dir().join("cut").join("waveforms"));
        let watchers = Watchers::new();
        let filmstrips = Filmstrips::new()
            .inspect_err(|error| tracing::warn!(%error, "timeline thumbnails unavailable"))
            .ok();
        let waveforms = Waveforms::new(cache)
            .inspect_err(|error| tracing::warn!(%error, "timeline waveforms unavailable"))
            .ok();
        if let Some(filmstrips) = &filmstrips {
            let repainting = watchers.clone();
            filmstrips.on_change(move || repainting.repaint());
        }
        if let Some(waveforms) = &waveforms {
            let repainting = watchers.clone();
            waveforms.on_change(move || repainting.repaint());
        }
        MediaPreviews(Rc::new(Inner {
            filmstrips,
            waveforms,
            watchers,
        }))
    }
}

impl MediaPreviews {
    pub(crate) fn same(&self, other: &MediaPreviews) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn filmstrips(&self) -> Option<&Filmstrips> {
        self.0.filmstrips.as_ref()
    }

    pub(crate) fn waveforms(&self) -> Option<&Waveforms> {
        self.0.waveforms.as_ref()
    }

    pub(crate) fn forget(&self, path: &Path) {
        if let Some(filmstrips) = &self.0.filmstrips {
            filmstrips.forget(path);
        }
        if let Some(waveforms) = &self.0.waveforms {
            waveforms.forget(path);
        }
    }

    pub(crate) fn watch(&self) {
        self.0.watchers.watch_current();
    }
}

impl std::fmt::Debug for MediaPreviews {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MediaPreviews")
            .field("filmstrips", &self.0.filmstrips)
            .field("waveforms", &self.0.waveforms)
            .finish_non_exhaustive()
    }
}
