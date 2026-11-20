use rikka_sync::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::thread::JoinHandle;

use rikka_gecko_protocol::{GeckoMessage, PROTOCOL, Target, WindowId};
use rikka_nested::{Event as NestedEvent, Nested, ViewId};
use rikka_types::Size;

use crate::error::{Error, Result};
use crate::outbox::Outbox;
use crate::process::supervise::{Exit, Watch};
use crate::runtime::dispatch;
use crate::runtime::event::RuntimeEvent;
use crate::runtime::handle::Runtime;
use crate::session::core::SessionCore;
use crate::session::dispatch as session_dispatch;
use crate::session::{CloseReason, Event, Session, SessionSettings};
use crate::threads;
use crate::threads::OwnedThreads;
use crate::wire::Link;

const UNSHOWN_VIEWPORT: Size = Size::new(1280.0, 720.0);

#[derive(Debug)]
pub(crate) struct RuntimeCore {
    pub(crate) link: Arc<Link>,
    nested: Option<Nested>,
    pub(crate) outbox: Outbox<RuntimeEvent>,
    pub(crate) ephemeral: bool,
    sessions: Mutex<Sessions>,
    version: Mutex<Option<String>>,
    minted: AtomicU64,
    watch: Mutex<Option<Watch>>,
    threads: OwnedThreads,
}

#[derive(Debug, Default)]
struct Sessions {
    windows: HashMap<WindowId, Weak<SessionCore>>,
    views: HashMap<ViewId, WindowId>,
}

impl RuntimeCore {
    pub(crate) fn new(link: Arc<Link>, nested: Nested, ephemeral: bool) -> RuntimeCore {
        RuntimeCore {
            link,
            nested: Some(nested),
            outbox: Outbox::default(),
            ephemeral,
            sessions: Mutex::new(Sessions::default()),
            version: Mutex::new(None),
            minted: AtomicU64::new(1),
            watch: Mutex::new(None),
            threads: OwnedThreads::default(),
        }
    }

    pub(crate) fn add_watch(&self, watch: Watch) {
        *self.watch.lock() = Some(watch);
    }

    pub(crate) fn adopt_thread(&self, thread: JoinHandle<()>) {
        self.threads.adopt(thread);
    }

    pub(crate) fn version(&self) -> Option<String> {
        self.version.lock().clone()
    }

    pub(crate) fn next_window_id(&self) -> WindowId {
        let number = self.minted.fetch_add(1, Ordering::Relaxed);
        WindowId::new(format!("rikka{number}")).expect("generated window id is non-empty")
    }

    pub(crate) fn open_session(
        core: &Arc<RuntimeCore>,
        window: WindowId,
        settings: SessionSettings,
    ) -> Result<Session> {
        if core.link.is_disconnected() {
            return Err(Error::EngineExited);
        }
        let settings = settings.forced_private(core.ephemeral);
        let target = Target::Window(window.clone());
        core.link
            .listen(target, &session_dispatch::subscribed_events(&settings));
        let nested = core
            .nested
            .as_ref()
            .expect("the display lives until the runtime is dropped");
        let view = nested.view(format!("rikka-view-{window}"));
        view.resize(settings.viewport().unwrap_or(UNSHOWN_VIEWPORT), 1.0);
        let init = settings.init();
        let session = Arc::new(SessionCore::new(
            window.clone(),
            &core.link,
            Arc::downgrade(core),
            view.clone(),
            settings,
        ));
        {
            let mut sessions = core.sessions.lock();
            sessions
                .windows
                .insert(window.clone(), Arc::downgrade(&session));
            sessions.views.insert(view.id(), window.clone());
        }
        core.link.open(&window, &init);
        Ok(Session::new(session, Runtime(Arc::clone(core))))
    }

    pub(crate) fn remove_window(&self, window: &WindowId) {
        let mut sessions = self.sessions.lock();
        sessions.windows.remove(window);
        sessions
            .views
            .retain(|_, view_window| view_window != window);
    }

    pub(crate) fn handle_message(core: &Arc<RuntimeCore>, message: GeckoMessage) {
        match message {
            GeckoMessage::Hello(hello) if hello.protocol != PROTOCOL => {
                tracing::error!(
                    engine = hello.protocol,
                    expected = PROTOCOL,
                    "engine protocol version mismatch"
                );
                core.disconnect(CloseReason::EngineExited);
                core.outbox.send(RuntimeEvent::Incompatible {
                    protocol: hello.protocol,
                });
            }
            GeckoMessage::Hello(hello) => {
                *core.version.lock() = Some(hello.version.clone());
                core.link.release_held(&Target::Runtime);
                core.outbox.send(RuntimeEvent::Started {
                    version: hello.version,
                });
            }
            GeckoMessage::Event(event) => match &event.target {
                Target::Runtime => dispatch::handle_message(core, &event),
                Target::Window(window) => match core.session(window) {
                    Some(session) => session_dispatch::handle_message(&session, &event),
                    None => core.reject_unhandled(&event),
                },
                Target::Named(_) => core.reject_unhandled(&event),
            },
            GeckoMessage::Reply(reply) => core.link.resolve(reply),
            GeckoMessage::Release(release) => core.link.release(release.callback),
            GeckoMessage::Ready(ready) => core.link.release_held(&Target::Window(ready.window)),
            GeckoMessage::Closed(closed) => {
                core.link.closed(&closed.window);
                if let Some(session) = core.session(&closed.window) {
                    session.end(CloseReason::Requested);
                }
            }
        }
    }

    pub(crate) fn disconnected(&self, reason: &str) {
        tracing::info!(%reason, "engine channel closed");
        self.disconnect(CloseReason::EngineExited);
    }

    pub(crate) fn exited(&self, exit: Exit) {
        self.disconnect(CloseReason::EngineExited);
        self.outbox.send(RuntimeEvent::Exited(exit));
        self.outbox.close();
    }

    pub(crate) fn nested(&self, event: NestedEvent) {
        match event {
            NestedEvent::FullscreenRequested { view, fullscreen } => {
                if let Some(session) = self.session_of(view) {
                    session.view.set_fullscreen(fullscreen);
                    session.change(|page| page.fullscreen = fullscreen);
                    session.outbox.send(Event::Fullscreen(fullscreen));
                }
            }
            NestedEvent::ActivationRequested { view } => {
                if let Some(session) = self.session_of(view) {
                    session.outbox.send(Event::FocusRequested);
                }
            }
            _ => {}
        }
    }

    fn disconnect(&self, reason: CloseReason) {
        self.link.disconnect();
        let sessions: Vec<Weak<SessionCore>> =
            self.sessions.lock().windows.values().cloned().collect();
        for session in sessions.iter().filter_map(Weak::upgrade) {
            session.end(reason);
        }
    }

    fn session(&self, window: &WindowId) -> Option<Arc<SessionCore>> {
        let weak = self.sessions.lock().windows.get(window).cloned()?;
        weak.upgrade()
    }

    fn session_of(&self, view: ViewId) -> Option<Arc<SessionCore>> {
        let window = self.sessions.lock().views.get(&view).cloned()?;
        self.session(&window)
    }

    fn reject_unhandled(&self, event: &rikka_gecko_protocol::EventMessage) {
        self.link
            .refuse(event.callback, format!("{} has no listener", event.target));
    }
}

impl Drop for RuntimeCore {
    fn drop(&mut self) {
        self.link.disconnect();
        let nested = self.nested.take();
        let engine_threads = self.threads.take();
        if let Some(watch) = self.watch.lock().take() {
            watch.part(move || {
                drop(nested);
                for thread in engine_threads {
                    threads::join(thread);
                }
            });
        }
    }
}
