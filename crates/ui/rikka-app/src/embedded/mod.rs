mod look;
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
use rikka_ui::{Command, Event, State, Ui, WindowRequest};
use rikka_ui_host::{Composer, VideoPlayback};
use rustc_hash::FxHashMap;

use crate::error::Result;
use crate::preload;
use crate::sender::{Route, Sender, WindowAlive};
use crate::wayland::settings::{self, SessionChange};

pub(crate) use mail::{Mail, Post};
pub use rikka_settings::session::appearance::Background;
pub use surface::Embedded;

use look::Look;
use mail::Mailbox;
use surface::Handlers;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SurfaceId(u64);

impl SurfaceId {
    #[must_use]
    pub fn get(self) -> u64 {
        self.0
    }
}

struct Hooks {
    handlers: Handlers,
    alive: WindowAlive,
    departed: bool,
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
    Leave(SurfaceId),
    Rejoin(SurfaceId),
    Wear(Box<Theme>),
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
    look: RefCell<Look>,
    next: Cell<u64>,
}

impl Embedding {
    pub fn new(device: ExternalDevice, i18n: I18nBuilder) -> Result<Embedding> {
        rikka_log::install("warn");
        i18n.embedded(rikka_widgets::CATALOGS).build()?.install();
        settings::apply_session_locale();
        let renderer = Renderer::new(&rikka_gpu::Gpu::builder().external(device).build()?)?;
        let (post, mailbox) = mail::mailbox();
        let session = Appearance::current();
        let look = Look::new(session.clone());
        preload::fonts(look.worn());
        let following = post.clone();
        settings::follow(
            move |change| following.send(Mail::Session(change)),
            Some(session),
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
            look: RefCell::new(look),
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
        let theme = self.look.borrow().worn().clone();
        let (ui, handlers) = surface.into_ui(Sender::new(route, alive.watch()), theme);
        let mut ui: Ui = ui.into();
        ui.arrive();
        self.defer(Deferred::Open {
            id,
            ui: Box::new(ui),
            hooks: Hooks {
                handlers,
                alive,
                departed: false,
            },
        });
        id
    }

    pub fn close(&self, id: SurfaceId) {
        self.defer(Deferred::Close(id));
    }

    pub fn resize(&self, id: SurfaceId, size: Size, scale: f32) {
        self.defer(Deferred::Resize { id, size, scale });
    }

    pub fn leave(&self, id: SurfaceId) {
        self.defer(Deferred::Leave(id));
    }

    pub fn rejoin(&self, id: SurfaceId) {
        self.defer(Deferred::Rejoin(id));
    }

    pub fn set_theme(&self, slug: Option<String>) {
        let worn = self.look.borrow_mut().set_theme(slug);
        if let Some(theme) = worn {
            self.defer(Deferred::Wear(Box::new(theme)));
        }
    }

    pub fn set_background(&self, background: Option<Background>) {
        let worn = self.look.borrow_mut().set_background(background);
        if let Some(theme) = worn {
            self.defer(Deferred::Wear(Box::new(theme)));
        }
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
            live.follow_exit(id);
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
                Deferred::Leave(id) => {
                    if let Some(composed) = live.composer.get_mut(&id) {
                        composed.ui.leave();
                    }
                }
                Deferred::Rejoin(id) => {
                    if let Some(composed) = live.composer.get_mut(&id) {
                        composed.ui.rejoin();
                    }
                    if let Some(hooks) = live.hooks.get_mut(&id) {
                        hooks.departed = false;
                    }
                }
                Deferred::Wear(theme) => wear(live, &theme),
            }
        }
    }

    fn collect_mail(&self, live: &mut Live) {
        while let Some(mail) = self.mailbox.take() {
            match mail {
                Mail::Message { surface, inbound } => live.composer.deliver(&surface, inbound),
                Mail::Session(SessionChange::Appearance { appearance, theme }) => {
                    let worn = self
                        .look
                        .borrow_mut()
                        .follow(*appearance, theme.map(|theme| *theme));
                    if let Some(theme) = worn {
                        wear(live, &theme);
                    }
                }
                Mail::Session(SessionChange::Language) => {}
            }
        }
    }
}

fn wear(live: &mut Live, theme: &Theme) {
    for (_, composed) in live.composer.iter_mut() {
        composed.ui.set_theme(theme.clone());
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
        match &hooks.handlers.close {
            Some(close) => ui.start(close.clone()),
            None => tracing::debug!("embedded surface asked to close without a close handler"),
        }
    }

    fn follow_exit(&mut self, id: SurfaceId) {
        let (Some(composed), Some(hooks)) = (self.composer.get_mut(&id), self.hooks.get_mut(&id))
        else {
            return;
        };
        let ui: &mut Ui = &mut composed.ui;
        if hooks.departed || !ui.is_leaving() || ui.needs_render() {
            return;
        }
        hooks.departed = true;
        if let Some(left) = &hooks.handlers.left {
            ui.start(left.clone());
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
