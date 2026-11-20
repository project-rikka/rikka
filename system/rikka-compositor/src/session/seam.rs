use std::path::Path;
use std::{fmt, io};

use rikka_compositor_reactor::{Closed, Reactor, Sender};
use rikka_seam::net::{SOCKET_ENV, SocketListener, socket_path_for};
use rikka_seam::{Call, Dispatcher, Seam, SeamError, Service, ServiceContext};

use crate::state::Compositor;

pub(crate) struct SeamHost {
    seam: Seam<Compositor>,
    socket: Option<SocketListener>,
}

struct Queue(Sender<Call>);

impl Dispatcher for Queue {
    fn enqueue(&self, call: Call) {
        if let Err(Closed(call)) = self.0.send(call) {
            call.fail(SeamError::Disconnected);
        }
    }
}

impl SeamHost {
    pub(crate) fn new(reactor: &Reactor<Compositor>) -> io::Result<Self> {
        let calls = reactor.mailbox(|compositor: &mut Compositor, call: Call| {
            compositor.answer(call);
        })?;
        Ok(Self {
            seam: Seam::new(Queue(calls)),
            socket: None,
        })
    }

    pub(crate) fn seam(&mut self) -> &mut Seam<Compositor> {
        &mut self.seam
    }

    pub(crate) fn register(
        &mut self,
        capability: &str,
        service: impl Service<Compositor>,
    ) -> Option<ServiceContext> {
        self.seam
            .register(service)
            .inspect_err(|error| tracing::error!("cannot register `{capability}`: {error}"))
            .ok()
    }

    pub(crate) fn listen(&mut self, display: &str) {
        let Some(path) = socket_path_for(display) else {
            return tracing::warn!(
                "no runtime directory; session settings unreachable (${SOCKET_ENV})"
            );
        };
        match SocketListener::serve(&path, self.seam.client()) {
            Ok(listener) => {
                tracing::info!("seam listening on {}", path.display());
                self.socket = Some(listener);
            }
            Err(error) => tracing::warn!("cannot listen for session settings: {error}"),
        }
    }

    pub(crate) fn socket(&self) -> Option<&Path> {
        self.socket.as_ref().map(SocketListener::path)
    }
}

impl fmt::Debug for SeamHost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SeamHost")
            .field("socket", &self.socket())
            .finish_non_exhaustive()
    }
}
