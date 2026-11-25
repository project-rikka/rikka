use std::cell::{Cell, RefCell};
use std::fmt;
use std::os::fd::AsFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::rc::{Rc, Weak};

use rikka_compositor_reactor::{Interest, Reactor, Token};
use rikka_wayland_wire::{
    Bindable, ClientId, Connection, Disconnect, Interface, Object, Peer, Resource,
};

use crate::builtins::CoreProtocol;
use crate::client::{Admission, Client};
use crate::dispatch::{Dispatcher, Served};
use crate::error::{Error, Result};
use crate::global::GlobalBuilder;
use crate::listen::{self, Listener, Socket};
use crate::module::Module;
use crate::options::{ClientOptions, Credentials};
use crate::shared::Core;
use crate::{service, upkeep};

type DisconnectHook<S> = Rc<dyn Fn(&mut S, &Client, &Disconnect)>;

pub struct DisplayHandle<S: 'static> {
    inner: Rc<DisplayInner<S>>,
}

pub(crate) struct DisplayInner<S: 'static> {
    pub(crate) core: Rc<Core>,
    pub(crate) reactor: Reactor<S>,
    pub(crate) deferred_destroys: Cell<Vec<Object>>,
    pub(crate) dirty: Cell<Vec<ClientId>>,
    dispatchers: RefCell<Vec<Option<Rc<dyn Dispatcher<S>>>>>,
    disconnect_hooks: RefCell<Vec<DisconnectHook<S>>>,
    maintenance: Token,
    scheduled: Cell<bool>,
}

impl<S: 'static> DisplayInner<S> {
    pub(crate) fn dispatcher(&self, interface: &Interface) -> Option<Rc<dyn Dispatcher<S>>> {
        self.dispatchers
            .borrow()
            .get(usize::from(interface.index))?
            .clone()
    }

    pub(crate) fn disconnected(&self, state: &mut S, client: &Client, reason: &Disconnect) {
        let hooks = self.disconnect_hooks.borrow().clone();
        for hook in hooks {
            hook(state, client, reason);
        }
    }

    fn schedule(&self) {
        if !self.scheduled.replace(true) {
            self.reactor.wake(self.maintenance);
        }
    }

    fn install(&self, interface: &Interface, dispatcher: Rc<dyn Dispatcher<S>>) {
        let index = usize::from(interface.index);
        let mut dispatchers = self.dispatchers.borrow_mut();
        if dispatchers.len() <= index {
            dispatchers.resize_with(index + 1, || None);
        }
        dispatchers[index] = Some(dispatcher);
    }
}

impl<S: 'static> Drop for DisplayInner<S> {
    fn drop(&mut self) {
        self.reactor.remove(self.maintenance);
    }
}

impl<S: 'static> DisplayHandle<S> {
    pub fn new(reactor: &Reactor<S>) -> Self {
        let inner = Rc::new_cyclic(|this: &Weak<DisplayInner<S>>| {
            let waker = this.clone();
            let timers = reactor.clone();
            let maintained = this.clone();
            let maintenance = reactor.task(move |state| {
                if let Some(display) = maintained.upgrade() {
                    upkeep::maintain(&display, state);
                    display.scheduled.set(false);
                }
            });
            DisplayInner {
                core: Rc::new(Core::new(
                    move || {
                        if let Some(display) = waker.upgrade() {
                            display.schedule();
                        }
                    },
                    Box::new(move |delay, task| {
                        timers.after(delay, move |_| task());
                    }),
                )),
                reactor: reactor.clone(),
                deferred_destroys: Cell::new(Vec::new()),
                dirty: Cell::new(Vec::new()),
                dispatchers: RefCell::new(Vec::new()),
                disconnect_hooks: RefCell::new(Vec::new()),
                maintenance,
                scheduled: Cell::new(false),
            }
        });
        let display = Self { inner };
        display.install(CoreProtocol);
        display
    }

    pub fn reactor(&self) -> &Reactor<S> {
        &self.inner.reactor
    }

    pub fn listen(&self, name: Option<&str>) -> Result<Socket> {
        listen::socket(self, name)
    }

    pub fn listen_at(&self, path: &Path, options: ClientOptions) -> Result<Socket> {
        listen::socket_at(self, path, options)
    }

    pub fn accept_on(&self, listener: UnixListener, options: ClientOptions) -> Result<Listener> {
        listener
            .set_nonblocking(true)
            .map_err(Error::io("making a listening socket non-blocking"))?;
        let listener = Rc::new(listener);
        let accepting = listener.clone();
        let this = Rc::downgrade(&self.inner);
        let token = self
            .inner
            .reactor
            .watch(listener.as_fd(), Interest::READABLE, move |_, _| {
                let Some(inner) = this.upgrade() else {
                    return;
                };
                let display = DisplayHandle { inner };
                listen::accept_all(&accepting, |stream| {
                    if let Err(error) = display.insert_client(stream, options.clone()) {
                        tracing::warn!("failed to add client: {error}");
                    }
                });
            })
            .map_err(Error::io("watching a listening socket"))?;
        let reactor = self.inner.reactor.clone();
        Ok(Listener::new(move || reactor.remove(token)))
    }

    pub fn insert_client(&self, stream: UnixStream, options: ClientOptions) -> Result<Client> {
        let credentials = rustix::net::sockopt::socket_peercred(&stream)
            .map(|credentials| Credentials {
                pid: credentials.pid.as_raw_nonzero().get(),
                uid: credentials.uid.as_raw(),
                gid: credentials.gid.as_raw(),
            })
            .map_err(|error| Error::Io {
                action: "reading the credentials of a client".to_owned(),
                source: error.into(),
            })?;
        let core = &self.inner.core;
        let id = core.allocate_client();
        let peer = Peer::new(id, Connection::new(stream));
        let waker = Rc::downgrade(core);
        peer.set_waker(move || {
            if let Some(core) = waker.upgrade() {
                core.mark_dirty(id);
            }
        });
        let client = Client::admit(Admission {
            peer,
            credentials,
            privileged: options.privileged(),
            xwayland: options.is_xwayland(),
            agent: options.is_agent(),
            security_context: options.context().cloned(),
            core: Rc::downgrade(core),
        });
        let this = Rc::downgrade(&self.inner);
        let watched = client.clone();
        let token = {
            let socket = client
                .peer()
                .connection()
                .socket()
                .expect("a connection is open until its client is torn down");
            self.inner
                .reactor
                .watch(socket.as_fd(), Interest::READABLE, move |state, ready| {
                    if let Some(display) = this.upgrade() {
                        service::ready(&display, state, &watched, ready);
                    }
                })
                .map_err(Error::io("watching a client socket"))?
        };
        client.watch().set(Some(token));
        core.admit(client.clone());
        tracing::debug!(
            client = id.raw(),
            pid = credentials.pid,
            privileged = client.privileged(),
            "client connected"
        );
        Ok(client)
    }

    pub fn serve<R: Resource>(
        &self,
        request: for<'m> fn(&mut S, R, R::Request<'m>),
    ) -> Served<S, R> {
        let served = Served::new(request);
        self.inner.install(R::INTERFACE, served.handlers());
        served
    }

    pub fn serves(&self, interface: &Interface) -> bool {
        self.inner.dispatcher(interface).is_some()
    }

    pub fn install(&self, module: impl Module<S>) {
        module.install(self);
    }

    pub fn global<R: Bindable>(&self) -> GlobalBuilder<'_, S, R> {
        GlobalBuilder::new(self)
    }

    pub fn client(&self, id: ClientId) -> Option<Client> {
        self.inner.core.client(id)
    }

    pub fn clients(&self) -> Rc<[Client]> {
        self.inner.core.clients()
    }

    pub fn serial(&self) -> u32 {
        self.inner.core.serial()
    }

    pub fn next_serial(&self) -> u32 {
        self.inner.core.next_serial()
    }

    pub fn on_disconnect(&self, hook: impl Fn(&mut S, &Client, &Disconnect) + 'static) {
        self.inner.disconnect_hooks.borrow_mut().push(Rc::new(hook));
    }

    pub(crate) fn core(&self) -> &Rc<Core> {
        &self.inner.core
    }
}

impl<S: 'static> Clone for DisplayHandle<S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<S: 'static> fmt::Debug for DisplayHandle<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DisplayHandle")
            .field("clients", &self.inner.core.clients().len())
            .field(
                "served",
                &self
                    .inner
                    .dispatchers
                    .borrow()
                    .iter()
                    .filter(|dispatcher| dispatcher.is_some())
                    .count(),
            )
            .finish()
    }
}
