use std::any::Any;
use std::cell::OnceCell;
use std::marker::PhantomData;

use rikka_clipboard::Clipboard;
use rikka_seam::SeamClient;

use crate::{Outputs, Typist};

pub struct Host {
    seam: SeamClient,
    outputs: Outputs,
    typist: Typist,
    clipboard: OnceCell<Option<Clipboard>>,
    held: Vec<Box<dyn Any>>,
}

pub struct Slot<T> {
    index: usize,
    held: PhantomData<fn() -> T>,
}

impl Host {
    pub(crate) fn new(seam: SeamClient) -> Host {
        Host {
            seam,
            outputs: Outputs::default(),
            typist: Typist::default(),
            clipboard: OnceCell::new(),
            held: Vec::new(),
        }
    }

    #[must_use]
    pub fn seam(&self) -> &SeamClient {
        &self.seam
    }

    #[must_use]
    pub fn outputs(&self) -> &Outputs {
        &self.outputs
    }

    #[must_use]
    pub fn typist(&self) -> &Typist {
        &self.typist
    }

    #[must_use]
    pub fn clipboard(&self) -> Option<Clipboard> {
        self.clipboard
            .get_or_init(|| {
                Clipboard::connect()
                    .inspect_err(|error| tracing::warn!("clipboard unavailable: {error}"))
                    .ok()
            })
            .as_ref()
            .map(Clipboard::share)
    }

    #[must_use]
    pub fn get<T: 'static>(&self, slot: Slot<T>) -> &T {
        self.held[slot.index]
            .downcast_ref()
            .expect("slot names a value of its type")
    }

    #[must_use]
    pub fn get_mut<T: 'static>(&mut self, slot: Slot<T>) -> &mut T {
        self.held[slot.index]
            .downcast_mut()
            .expect("slot names a value of its type")
    }

    pub(crate) fn hold<T: 'static>(&mut self, value: T) -> Slot<T> {
        self.held.push(Box::new(value));
        Slot {
            index: self.held.len() - 1,
            held: PhantomData,
        }
    }
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host")
            .field("outputs", &self.outputs)
            .field("held", &self.held.len())
            .finish_non_exhaustive()
    }
}

impl<T> Clone for Slot<T> {
    fn clone(&self) -> Slot<T> {
        *self
    }
}

impl<T> Copy for Slot<T> {}

impl<T> std::fmt::Debug for Slot<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Slot").field(&self.index).finish()
    }
}
