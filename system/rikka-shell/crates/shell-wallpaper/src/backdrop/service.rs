use rikka_seam::{
    Call, EventInfo, Manifest, Method, MethodInfo, SeamError, Service, ServiceContext,
};
use rikka_session::wallpaper::{
    CAPABILITY, DisplayedWallpaper, LIST, LISTED, REFRESH, RESERVE, RESERVED, SHOW, SHOWING,
    VERSION,
};
use shell_kit::Host;

use crate::{LibraryWatcher, Shelf};

#[derive(Debug)]
pub(super) struct WallpaperService {
    shelf: Shelf,
    library_watcher: Option<LibraryWatcher>,
    seam: Option<ServiceContext>,
    showing: Option<DisplayedWallpaper>,
    reserved: f32,
}

impl WallpaperService {
    #[must_use]
    pub(crate) fn new() -> WallpaperService {
        WallpaperService {
            shelf: Shelf::new(),
            library_watcher: None,
            seam: None,
            showing: None,
            reserved: 0.0,
        }
    }

    #[must_use]
    pub(crate) fn showing(mut self, id: Option<String>) -> WallpaperService {
        self.showing = id.and_then(|id| self.shelf.shown(&id));
        self
    }

    #[must_use]
    pub(crate) fn below(mut self, reserved: f32) -> WallpaperService {
        self.reserved = reserved;
        self
    }

    fn list(&self, call: Call) {
        let Some(library_watcher) = &self.library_watcher else {
            return call.fail(closed());
        };
        let (_, responder) = call.defer();
        library_watcher.list(responder);
    }

    fn refresh(&self, call: Call) {
        match &self.library_watcher {
            Some(library_watcher) => {
                library_watcher.announce();
                call.done();
            }
            None => call.fail(closed()),
        }
    }

    fn show(&mut self, call: Call) {
        let id = match call.decode::<Option<String>>() {
            Ok(id) => id,
            Err(error) => return call.fail(error),
        };
        let shown = match id {
            Some(id) => match self.shelf.shown(&id) {
                Some(shown) => Some(shown),
                None => {
                    let refusal = format!("`{id}` is not a wallpaper of this session");
                    tracing::warn!("wallpaper change refused: {refusal}");
                    return call.fail(SeamError::InvalidParams(refusal));
                }
            },
            None => None,
        };
        if self.showing == shown {
            return call.done();
        }
        self.showing = shown;
        if let Some(seam) = &self.seam
            && let Err(error) = seam.publisher().emit(SHOWING, &self.showing)
        {
            return call.fail(error);
        }
        call.done();
    }

    fn reserve(&mut self, call: Call) {
        let reserved = match call.decode::<f32>() {
            Ok(reserved) => reserved,
            Err(error) => return call.fail(error),
        };
        if self.reserved == reserved {
            return call.done();
        }
        self.reserved = reserved;
        if let Some(seam) = &self.seam
            && let Err(error) = seam.publisher().emit(RESERVED, &reserved)
        {
            return call.fail(error);
        }
        call.done();
    }
}

impl Service<Host> for WallpaperService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, VERSION)
            .summary("Wallpaper and wallpaper library")
            .method(MethodInfo::new(LIST).summary("Lists the wallpaper library"))
            .method(MethodInfo::new(REFRESH).summary("Rescans the library and emits `listed`"))
            .method(MethodInfo::new(SHOW).summary(
                "Crosses every wallpaper surface to the wallpaper named, or to nothing. \
                 The session's own choice is `rikka.appearance/wallpaper`",
            ))
            .method(MethodInfo::new(SHOWING).summary("Returns the current wallpaper id"))
            .method(
                MethodInfo::new(RESERVE)
                    .summary("How much of the top of every output the backdrop leaves alone"),
            )
            .method(MethodInfo::new(RESERVED).summary("Returns the reserved top strip height"))
            .event(
                EventInfo::new(LISTED)
                    .summary("Library or thumbnail changed")
                    .latest(),
            )
            .event(
                EventInfo::new(SHOWING)
                    .summary("Current wallpaper changed")
                    .latest(),
            )
            .event(
                EventInfo::new(RESERVED)
                    .summary("Reserved top strip changed")
                    .latest(),
            )
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.library_watcher = Some(self.shelf.announcing(seam.publisher().clone()));
        self.seam = Some(seam);
    }

    fn call(&mut self, _: &mut Host, method: Method, call: Call) {
        match &*method {
            LIST => self.list(call),
            REFRESH => self.refresh(call),
            SHOW => self.show(call),
            SHOWING => call.encode(&self.showing),
            RESERVE => self.reserve(call),
            RESERVED => call.encode(&self.reserved),
            _ => call.unknown_method(),
        }
    }
}

fn closed() -> SeamError {
    SeamError::Unavailable("wallpaper library not loaded yet".to_owned())
}
