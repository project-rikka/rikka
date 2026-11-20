use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rikka_ui::{RuntimeMessage, State, Update, WindowRef};

use crate::error::SendError;
use crate::tasks::{LoopEvent, LoopProxy, TaskMessage, WindowId};

#[derive(Debug)]
pub(crate) struct WindowAlive(Arc<AtomicBool>);

impl WindowAlive {
    pub(crate) fn open() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }

    pub(crate) fn watch(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.0)
    }

    pub(crate) fn close(&self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Drop for WindowAlive {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[derive(Clone)]
pub(crate) struct Route {
    pub proxy: LoopProxy,
    pub window: WindowId,
}

impl Route {
    fn window(&self) -> WindowRef {
        WindowRef::new(u64::from(self.window))
    }

    fn deliver(&self, inbound: RuntimeMessage) -> bool {
        self.proxy
            .send_event(LoopEvent::Message(TaskMessage {
                window: self.window,
                inbound,
            }))
            .is_ok()
    }
}

pub struct Sender<S: State> {
    route: Route,
    alive: Arc<AtomicBool>,
    state: PhantomData<fn(S)>,
}

impl<S: State> Sender<S> {
    pub(crate) fn new(route: Route, alive: Arc<AtomicBool>) -> Self {
        Self {
            route,
            alive,
            state: PhantomData,
        }
    }

    #[must_use]
    pub fn window(&self) -> WindowRef {
        self.route.window()
    }

    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    pub fn send(&self, update: impl FnOnce(&mut S) + Send + 'static) -> Result<(), SendError> {
        self.send_to(update)
    }

    pub fn send_to<T: 'static>(
        &self,
        update: impl FnOnce(&mut T) + Send + 'static,
    ) -> Result<(), SendError> {
        if !self.is_alive() {
            return Err(SendError);
        }
        match self
            .route
            .deliver(RuntimeMessage::Update(Update::new::<T>(update)))
        {
            true => Ok(()),
            false => Err(SendError),
        }
    }
}

impl<S: State> Clone for Sender<S> {
    fn clone(&self) -> Self {
        Self::new(self.route.clone(), Arc::clone(&self.alive))
    }
}

impl<S: State> std::fmt::Debug for Sender<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sender")
            .field("alive", &self.is_alive())
            .finish_non_exhaustive()
    }
}
