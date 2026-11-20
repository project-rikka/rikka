use std::fmt;
use std::os::fd::AsFd;

use rikka_compositor_reactor::{Interest, Token};
use rikka_compositor_server::ClientOptions;
use rikka_compositor_xwm::{Xwayland, Xwm, XwmEvent};

use crate::context::{InputCtx, WindowsCtx, windows_ctx};
use crate::context::{SessionCtx, input_ctx};

pub(super) struct XServer {
    xwayland: Xwayland,
    starting: Option<Token>,
    exit: Token,
    manager: Option<(Xwm, Token)>,
    events: Vec<XwmEvent>,
}

impl XServer {
    pub(super) fn display_number(&self) -> u32 {
        self.xwayland.display_number()
    }

    pub(super) fn manager(&self) -> Option<&Xwm> {
        self.manager.as_ref().map(|(manager, _)| manager)
    }

    pub(super) fn manager_mut(&mut self) -> Option<&mut Xwm> {
        self.manager.as_mut().map(|(manager, _)| manager)
    }
}

impl fmt::Debug for XServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("XServer")
            .field("display", &self.xwayland.display())
            .field("managed", &self.manager.is_some())
            .finish_non_exhaustive()
    }
}

impl SessionCtx<'_> {
    pub(crate) fn start_xwayland(&mut self) {
        let (xwayland, wayland) = match Xwayland::launch().spawn() {
            Ok(spawned) => spawned,
            Err(error) => {
                return tracing::warn!("cannot start Xwayland: {error}");
            }
        };
        if let Err(error) = self
            .core
            .display
            .insert_client(wayland, ClientOptions::new().xwayland())
        {
            return tracing::warn!("Xwayland cannot connect to the display: {error}");
        }
        let reactor = &self.core.reactor;
        let starting = reactor.watch(xwayland.ready_fd(), Interest::READABLE, |compositor, _| {
            input_ctx!(compositor).xwayland_announced();
        });
        let exit = reactor.watch(xwayland.exit_fd(), Interest::READABLE, |compositor, _| {
            windows_ctx!(compositor).xwayland_exited();
        });
        match (starting, exit) {
            (Ok(starting), Ok(exit)) => {
                self.x11.server = Some(XServer {
                    xwayland,
                    starting: Some(starting),
                    exit,
                    manager: None,
                    events: Vec::new(),
                });
            }
            (starting, exit) => {
                for token in [starting.ok(), exit.ok()].into_iter().flatten() {
                    reactor.remove(token);
                }
                tracing::warn!("cannot watch Xwayland; stopping it");
            }
        }
    }
}

impl InputCtx<'_> {
    fn xwayland_announced(&mut self) {
        let Some(server) = &mut self.x11.server else {
            return;
        };
        let connection = match server.xwayland.ready() {
            Ok(Some(connection)) => connection,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!("Xwayland failed during startup: {error}");
                return windows_ctx!(self).stop_xwayland();
            }
        };
        if let Some(token) = server.starting.take() {
            self.core.reactor.remove(token);
        }
        let manager = match Xwm::start(connection) {
            Ok(manager) => manager,
            Err(error) => {
                tracing::warn!("cannot start X11 window manager: {error}");
                return windows_ctx!(self).stop_xwayland();
            }
        };
        let watched =
            self.core
                .reactor
                .watch(manager.as_fd(), Interest::READABLE, |compositor, _| {
                    windows_ctx!(compositor).x11_happened();
                });
        match watched {
            Ok(token) => {
                tracing::info!("Xwayland ready on {}", server.xwayland.display());
                server.manager = Some((manager, token));
                self.bridge_selections_to_x11();
            }
            Err(error) => {
                tracing::warn!("cannot watch X11 window manager connection: {error}");
                windows_ctx!(self).stop_xwayland();
            }
        }
    }
}

impl WindowsCtx<'_> {
    fn x11_happened(&mut self) {
        let Some(server) = self.x11.server.as_mut() else {
            return;
        };
        let Some((manager, _)) = server.manager.as_mut() else {
            return;
        };
        let mut events = std::mem::take(&mut server.events);
        if let Err(error) = manager.dispatch_into(&mut events) {
            tracing::warn!("X11 window manager lost connection to Xwayland: {error}");
            return self.stop_xwayland();
        }
        for event in events.drain(..) {
            self.x11_event(event);
        }
        if let Some(server) = self.x11.server.as_mut() {
            server.events = events;
        }
    }

    pub(crate) fn flush_x11(&mut self) {
        let Some(manager) = self.x11.server.as_mut().and_then(XServer::manager_mut) else {
            return;
        };
        if let Err(error) = manager.flush() {
            tracing::warn!("X11 window manager lost connection to Xwayland: {error}");
            self.stop_xwayland();
        }
    }

    fn xwayland_exited(&mut self) {
        let Some(server) = &mut self.x11.server else {
            return;
        };
        let Some(status) = server.xwayland.exited() else {
            return;
        };
        tracing::warn!("Xwayland exited unexpectedly: {status}");
        self.stop_xwayland();
    }

    fn stop_xwayland(&mut self) {
        let Some(server) = self.x11.server.take() else {
            return;
        };
        let reactor = &self.core.reactor;
        for token in server
            .starting
            .into_iter()
            .chain([server.exit])
            .chain(server.manager.as_ref().map(|(_, token)| *token))
        {
            reactor.remove(token);
        }
        self.forget_x11_windows();
    }
}
