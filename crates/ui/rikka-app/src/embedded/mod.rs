mod mail;
mod surface;

use std::cell::{Cell, RefCell, RefMut};

use rikka_appearance::Appearance;
use rikka_clipboard::Clipboard;
use rikka_gpu::ExternalDevice;
use rikka_i18n::I18nBuilder;
use rikka_renderer::{ExternalImage, Renderer};
use rikka_theme::Theme;
use rikka_types::{CursorShape, Rect, Size};
use rikka_ui::{Command, Event, Handler, State, Ui, WindowRequest};
use rikka_ui_host::{Composer, VideoPlayback};
use rustc_hash::FxHashMap;

use crate::error::Result;
use crate::preload;
use crate::sender::{Route, Sender, WindowAlive};
use crate::wayland::settings::{self, SessionChange};

pub(crate) use mail::{Mail, Post};
pub use surface::Embedded;

use mail::Mailbox;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SurfaceId(u64);

impl SurfaceId {
    #[must_use]
    pub fn get(self) -> u64 {
        self.0
    }
}

struct Hooks {
    close: Option<Handler>,
    alive: WindowAlive,
}

enum Deferred {
    Open {
        id: SurfaceId,
        ui: Box<Ui>,
        hooks: Hooks,
    },
    Close(SurfaceId),
    Resize {
        id: SurfaceId,
        size: Size,
        scale: f32,
    },
}

struct Live {
    composer: Composer<SurfaceId>,
    hooks: FxHashMap<SurfaceId, Hooks>,
}

pub struct Embedding {
    live: RefCell<Live>,
    deferred: RefCell<Vec<Deferred>>,
    post: Post,
    mailbox: Mailbox,
    theme: RefCell<Theme>,
    next: Cell<u64>,
}

impl Embedding {
    pub fn new(device: ExternalDevice, i18n: I18nBuilder) -> Result<Embedding> {
        rikka_log::install("warn");
        i18n.embedded(rikka_widgets::CATALOGS).build()?.install();
        settings::apply_session_locale();
        let renderer = Renderer::new(&rikka_gpu::Gpu::builder().external(device).build()?)?;
        let (post, mailbox) = mail::mailbox();
        let worn = Appearance::current();
        let theme = worn.theme();
        preload::fonts(&theme);
        let following = post.clone();
        settings::follow(
            move |change| following.send(Mail::Session(change)),
            Some(worn),
            true,
        );
        match Clipboard::connect() {
            Ok(clipboard) => rikka_ui::clipboard::install_session(clipboard),
            Err(error) => tracing::warn!("failed to open session clipboard: {error}"),
        }
        Ok(Embedding {
            live: RefCell::new(Live {
                composer: Composer::new(renderer, post.clone()),
                hooks: FxHashMap::default(),
            }),
            deferred: RefCell::new(Vec::new()),
            post,
            mailbox,
            theme: RefCell::new(theme),
            next: Cell::new(1),
        })
    }

    #[must_use]
    pub fn plays_video(self, videos: impl VideoPlayback + 'static) -> Embedding {
        self.live
            .borrow_mut()
            .composer
            .plays_video(Box::new(videos));
        self
    }

    pub fn open<S: State>(&self, surface: Embedded<S>) -> SurfaceId {
        let id = SurfaceId(self.next.get());
        self.next.set(id.0 + 1);
        let alive = WindowAlive::open();
        let route = Route::Embedded {
            post: self.post.clone(),
            surface: id,
        };
        let theme = self.theme.borrow().clone();
        let (ui, close) = surface.into_ui(Sender::new(route, alive.watch()), theme);
        self.defer(Deferred::Open {
            id,
            ui: Box::new(ui.into()),
            hooks: Hooks { close, alive },
        });
        id
    }

    pub fn close(&self, id: SurfaceId) {
        self.defer(Deferred::Close(id));
    }

    pub fn resize(&self, id: SurfaceId, size: Size, scale: f32) {
        self.defer(Deferred::Resize { id, size, scale });
    }

    pub fn handle_event(&self, id: SurfaceId, event: Event) -> bool {
        let handled = {
            let mut live = self.enter();
            let Some(composed) = live.composer.get_mut(&id) else {
                return false;
            };
            let handled = composed.ui.handle_event(event);
            live.serve(id);
            handled
        };
        self.settle();
        handled
    }

    #[must_use]
    pub fn cursor(&self, id: SurfaceId) -> CursorShape {
        self.live
            .borrow()
            .composer
            .get(&id)
            .map_or(CursorShape::Default, |composed| composed.ui.cursor())
    }

    #[must_use]
    pub fn caret(&self, id: SurfaceId) -> Option<Rect> {
        self.live.borrow().composer.get(&id)?.ui.caret()
    }

    pub unsafe fn draw(&self, id: SurfaceId, image: ExternalImage) -> Result<()> {
        let drawn = {
            let mut live = self.enter();
            live.serve(id);
            let drawn = unsafe { live.composer.draw(&id, image) };
            live.serve(id);
            drawn
        };
        self.settle();
        Ok(drawn?)
    }

    fn enter(&self) -> RefMut<'_, Live> {
        let mut live = self
            .live
            .try_borrow_mut()
            .expect("an embedding is entered from one of its own callbacks");
        self.apply_deferred(&mut live);
        self.collect_mail(&mut live);
        live
    }

    fn defer(&self, deferred: Deferred) {
        self.deferred.borrow_mut().push(deferred);
        self.settle();
    }

    fn settle(&self) {
        if let Ok(mut live) = self.live.try_borrow_mut() {
            self.apply_deferred(&mut live);
        }
    }

    fn apply_deferred(&self, live: &mut Live) {
        let deferred = std::mem::take(&mut *self.deferred.borrow_mut());
        for change in deferred {
            match change {
                Deferred::Open { id, ui, hooks } => {
                    live.composer.insert(id, *ui);
                    live.hooks.insert(id, hooks);
                }
                Deferred::Close(id) => {
                    if let Some(hooks) = live.hooks.remove(&id) {
                        hooks.alive.close();
                    }
                    drop(live.composer.remove(&id));
                }
                Deferred::Resize { id, size, scale } => {
                    if let Some(composed) = live.composer.get_mut(&id) {
                        composed.ui.set_viewport(size, scale);
                        composed.ui.request_redraw();
                    }
                }
            }
        }
    }

    fn collect_mail(&self, live: &mut Live) {
        while let Some(mail) = self.mailbox.take() {
            match mail {
                Mail::Message { surface, inbound } => live.composer.deliver(&surface, inbound),
                Mail::Session(SessionChange::Theme(theme)) => self.retheme(live, *theme),
                Mail::Session(SessionChange::Language) => {}
            }
        }
    }

    fn retheme(&self, live: &mut Live, theme: Theme) {
        if *self.theme.borrow() == theme {
            return;
        }
        for (_, composed) in live.composer.iter_mut() {
            composed.ui.set_theme(theme.clone());
        }
        *self.theme.borrow_mut() = theme;
    }
}

impl Live {
    fn serve(&mut self, id: SurfaceId) {
        let (Some(composed), Some(hooks)) = (self.composer.get_mut(&id), self.hooks.get(&id))
        else {
            return;
        };
        let ui: &mut Ui = &mut composed.ui;
        let mut asked = matches!(ui.take_unclaimed_command(), Some(Command::Close));
        for request in ui.take_window_requests() {
            match request {
                WindowRequest::Close | WindowRequest::AskToClose => asked = true,
                request => {
                    tracing::debug!("{request:?} does not apply to an embedded surface");
                }
            }
        }
        if !asked {
            return;
        }
        match &hooks.close {
            Some(close) => ui.start(close.clone()),
            None => tracing::debug!("embedded surface asked to close without a close handler"),
        }
    }
}

impl Drop for Embedding {
    fn drop(&mut self) {
        for hooks in self.live.get_mut().hooks.values() {
            hooks.alive.close();
        }
    }
}

impl std::fmt::Debug for Embedding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Embedding").finish_non_exhaustive()
    }
}
