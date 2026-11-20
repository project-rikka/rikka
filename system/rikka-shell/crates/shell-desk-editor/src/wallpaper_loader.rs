use std::time::{Duration, Instant};

use futures_util::stream::BoxStream;
use rikka_ui::ImageSource;
use shell_wallpaper::{self as wallpaper, Media};

const LINGER: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub(super) struct WallpaperLoader {
    cover: Option<(u32, u32)>,
    current: Option<LoadedWallpaper>,
    before: Option<LoadedWallpaper>,
    loading: Option<Option<String>>,
    serial: u64,
}

#[derive(Debug)]
struct LoadedWallpaper {
    id: Option<String>,
    media: Option<Media>,
    serial: u64,
    fades: bool,
    since: Instant,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Pictures {
    pub(crate) under: Option<ImageSource>,
    pub(crate) over: Option<Over>,
}

#[derive(Clone, Debug)]
pub(crate) struct Over {
    pub(crate) source: ImageSource,
    pub(crate) serial: u64,
    pub(super) fades: bool,
}

impl WallpaperLoader {
    #[must_use]
    pub(crate) fn new(cover: Option<(u32, u32)>, showing: Option<String>) -> WallpaperLoader {
        let shared = showing
            .as_deref()
            .and_then(wallpaper::file_of)
            .and_then(|path| wallpaper::shared(&path, cover));
        let current = shared.map(|media| LoadedWallpaper {
            id: showing,
            media: Some(media),
            serial: 0,
            fades: false,
            since: Instant::now(),
        });
        WallpaperLoader {
            cover,
            current,
            before: None,
            loading: None,
            serial: 0,
        }
    }

    #[must_use]
    pub(crate) fn cover(&self) -> Option<(u32, u32)> {
        self.cover
    }

    #[must_use]
    pub(crate) fn wanting(&mut self, showing: Option<&str>) -> Option<Option<String>> {
        let current = self.current.as_ref().map(|current| current.id.as_deref());
        let loading = self.loading.as_ref().map(Option::as_deref);
        if current == Some(showing) || loading == Some(showing) {
            return None;
        }
        let id = showing.map(str::to_owned);
        self.loading = Some(id.clone());
        Some(id)
    }

    pub(crate) fn finish_loading(&mut self, id: Option<String>, media: Option<Media>) {
        if self.loading.as_ref() != Some(&id) {
            return;
        }
        self.loading = None;
        self.serial += 1;
        let first = self.current.is_none();
        let before = self.current.take().filter(|current| current.is_ready());
        self.before = before;
        self.current = Some(LoadedWallpaper {
            id,
            media,
            serial: self.serial,
            fades: !first,
            since: Instant::now(),
        });
    }

    pub(crate) fn settle(&mut self) -> bool {
        let settled = self.before.is_some()
            && self
                .current
                .as_ref()
                .is_some_and(|current| current.is_ready() && current.since.elapsed() >= LINGER);
        if settled {
            self.before = None;
        }
        settled
    }

    #[must_use]
    pub(crate) fn unready(&self) -> Option<BoxStream<'static, ()>> {
        let media = self.current.as_ref()?.media.as_ref()?;
        (!media.is_ready()).then(|| media.changes())
    }

    #[must_use]
    pub(crate) fn lingers_for(&self) -> Option<Duration> {
        let current = self.current.as_ref().filter(|current| current.is_ready())?;
        self.before
            .is_some()
            .then(|| LINGER.saturating_sub(current.since.elapsed()))
    }

    #[must_use]
    pub(crate) fn shown(&self, showing: Option<&str>) -> Option<String> {
        match &self.current {
            Some(current) if current.is_ready() => current.id.clone(),
            _ => match &self.before {
                Some(before) => before.id.clone(),
                None => showing.map(str::to_owned),
            },
        }
    }

    #[must_use]
    pub(crate) fn readiness(&self) -> (Option<u64>, bool) {
        (
            self.current.as_ref().map(|current| current.serial),
            self.current.as_ref().is_some_and(LoadedWallpaper::is_ready),
        )
    }

    #[must_use]
    pub(crate) fn pictures(&self) -> Pictures {
        let current = self.current.as_ref().filter(|current| current.is_ready());
        let under = match current {
            Some(current) if !current.fades => None,
            _ => self.before.as_ref().and_then(LoadedWallpaper::source),
        };
        Pictures {
            under,
            over: current.and_then(|current| {
                Some(Over {
                    source: current.source()?,
                    serial: current.serial,
                    fades: current.fades,
                })
            }),
        }
    }
}

impl LoadedWallpaper {
    fn is_ready(&self) -> bool {
        self.media.as_ref().is_none_or(Media::is_ready)
    }

    fn source(&self) -> Option<ImageSource> {
        let media = self.media.as_ref()?;
        media.is_ready().then(|| media.picture().source())
    }
}
