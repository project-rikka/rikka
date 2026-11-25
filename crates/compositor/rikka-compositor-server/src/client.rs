use std::any::Any;
use std::cell::Cell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::{Rc, Weak};

use rikka_compositor_protocols::wl_display::{self, WlDisplay};
use rikka_compositor_reactor::Token;
use rikka_wayland_wire::{ClientId, DISPLAY_ID, Disconnect, Peer, Resource};

use crate::options::{Credentials, SecurityContext};
use crate::shared::Core;

#[derive(Clone)]
pub struct Client(Rc<ClientInner>);

pub(crate) struct ClientInner {
    peer: Rc<Peer>,
    display: WlDisplay,
    credentials: Credentials,
    privileged: bool,
    xwayland: bool,
    agent: bool,
    security_context: Option<SecurityContext>,
    core: Weak<Core>,
    watch: Cell<Option<Token>>,
    writable: Cell<bool>,
}

pub(crate) struct Admission {
    pub(crate) peer: Rc<Peer>,
    pub(crate) credentials: Credentials,
    pub(crate) privileged: bool,
    pub(crate) xwayland: bool,
    pub(crate) agent: bool,
    pub(crate) security_context: Option<SecurityContext>,
    pub(crate) core: Weak<Core>,
}

impl Client {
    pub(crate) fn admit(admission: Admission) -> Self {
        let display = admission
            .peer
            .insert(DISPLAY_ID, &wl_display::INTERFACE, 1)
            .ok()
            .and_then(WlDisplay::from_object)
            .expect("an empty object table accepts the display object");
        let inner = Rc::new(ClientInner {
            peer: admission.peer,
            display,
            credentials: admission.credentials,
            privileged: admission.privileged,
            xwayland: admission.xwayland,
            agent: admission.agent,
            security_context: admission.security_context,
            core: admission.core,
            watch: Cell::new(None),
            writable: Cell::new(false),
        });
        let owner: Weak<dyn Any> = Rc::downgrade(&inner) as Weak<dyn Any>;
        inner.peer.set_owner(owner);
        Self(inner)
    }

    pub fn from_resource(resource: &impl Resource) -> Option<Self> {
        let owner = resource.object().peer().owner()?;
        owner.downcast::<ClientInner>().ok().map(Self)
    }

    pub fn id(&self) -> ClientId {
        self.0.peer.id()
    }

    pub fn credentials(&self) -> Credentials {
        self.0.credentials
    }

    pub fn privileged(&self) -> bool {
        self.0.privileged
    }

    pub fn is_xwayland(&self) -> bool {
        self.0.xwayland
    }

    pub fn is_agent(&self) -> bool {
        self.0.agent
    }

    pub fn security_context(&self) -> Option<&SecurityContext> {
        self.0.security_context.as_ref()
    }

    pub fn alive(&self) -> bool {
        self.0.peer.is_open()
    }

    pub fn disconnect_reason(&self) -> Option<Disconnect> {
        self.0.peer.closing()
    }

    pub fn create<R: Resource>(&self, version: u32) -> R {
        let object = match self.0.peer.allocate(R::INTERFACE, version) {
            Some(object) => object,
            None => {
                self.post_display_error(wl_display::Error::NoMemory, "no free object ids");
                self.0.peer.detached(R::INTERFACE, version)
            }
        };
        R::from_object(object).expect("an object created for R has R's interface")
    }

    pub fn object<R: Resource>(&self, id: u32) -> Option<R> {
        R::from_object(self.0.peer.object(id)?)
    }

    pub fn destroy(&self, resource: &impl Resource) {
        let object = resource.object();
        if object.client() != self.id() || !object.alive() {
            return;
        }
        object.expire();
        self.0.peer.defer_destroy(object.clone());
    }

    pub fn kill(&self, message: &str) {
        self.post_display_error(wl_display::Error::Implementation, message);
    }

    pub fn disconnect(&self) {
        self.0.peer.close(Disconnect::Requested);
    }

    pub(crate) fn post_display_error(&self, error: wl_display::Error, message: &str) {
        self.0.peer.post_error(
            wl_display::INTERFACE.name,
            DISPLAY_ID,
            error.code(),
            message,
        );
    }

    pub(crate) fn peer(&self) -> &Rc<Peer> {
        &self.0.peer
    }

    pub(crate) fn display(&self) -> &WlDisplay {
        &self.0.display
    }

    pub(crate) fn core(&self) -> Option<Rc<Core>> {
        self.0.core.upgrade()
    }

    pub(crate) fn watch(&self) -> &Cell<Option<Token>> {
        &self.0.watch
    }

    pub(crate) fn writable(&self) -> &Cell<bool> {
        &self.0.writable
    }
}

impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Eq for Client {}

impl Hash for Client {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Client")
            .field("id", &self.id().raw())
            .field("credentials", &self.0.credentials)
            .field("privileged", &self.0.privileged)
            .field("security_context", &self.0.security_context)
            .field("alive", &self.alive())
            .finish()
    }
}
