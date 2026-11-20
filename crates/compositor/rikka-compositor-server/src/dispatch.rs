use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use rikka_wayland_wire::{Header, Object, Resource, Signature};

use crate::client::Client;
use crate::service;

type Request<S, R> = for<'m> fn(&mut S, R, <R as Resource>::Request<'m>);
type Hook<S, R> = fn(&mut S, R);

pub struct Served<S, R: Resource> {
    handlers: Rc<Handlers<S, R>>,
}

pub(crate) struct Handlers<S, R: Resource> {
    request: Request<S, R>,
    destroyed: Cell<Hook<S, R>>,
    bound: Cell<Hook<S, R>>,
}

impl<S, R: Resource> Served<S, R> {
    pub(crate) fn new(request: Request<S, R>) -> Self {
        Self {
            handlers: Rc::new(Handlers {
                request,
                destroyed: Cell::new(|_, _| {}),
                bound: Cell::new(|_, _| {}),
            }),
        }
    }

    pub(crate) fn handlers(&self) -> Rc<Handlers<S, R>> {
        self.handlers.clone()
    }

    pub fn destroyed(&self, destroyed: fn(&mut S, R)) -> &Self {
        self.handlers.destroyed.set(destroyed);
        self
    }

    pub fn bound(&self, bound: fn(&mut S, R)) -> &Self {
        self.handlers.bound.set(bound);
        self
    }
}

impl<S, R: Resource> fmt::Debug for Served<S, R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Served")
            .field("interface", &R::INTERFACE.name)
            .finish_non_exhaustive()
    }
}

pub(crate) trait Dispatcher<S> {
    fn request(
        &self,
        state: &mut S,
        client: &Client,
        object: Object,
        header: Header,
        signature: Signature,
    );

    fn destroyed(&self, state: &mut S, object: Object);

    fn bound(&self, state: &mut S, object: Object);
}

impl<S, R: Resource> Dispatcher<S> for Handlers<S, R> {
    fn request(
        &self,
        state: &mut S,
        client: &Client,
        object: Object,
        header: Header,
        signature: Signature,
    ) {
        service::request::<S, R>(state, client, object, header, signature, self.request);
    }

    fn destroyed(&self, state: &mut S, object: Object) {
        if let Some(resource) = R::from_object(object) {
            (self.destroyed.get())(state, resource);
        }
    }

    fn bound(&self, state: &mut S, object: Object) {
        if let Some(resource) = R::from_object(object) {
            (self.bound.get())(state, resource);
        }
    }
}
