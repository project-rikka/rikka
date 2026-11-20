use std::sync::atomic::{AtomicU64, Ordering};

use rikka_sync::{Mutex, RwLock};
use rustc_hash::FxHashMap;

use crate::error::{Error, Resource, Result};
use crate::texture::TextureId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Picture,
    Canvas,
    Imported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mips {
    Fixed,
    Grows,
    Asked,
}

#[derive(Clone, Copy, Debug)]
pub struct Listing {
    pub slot: u32,
    pub extent: [u32; 2],
    pub kind: Kind,
    pub mips: Mips,
    pub texel_bytes: Option<u32>,
}

impl Listing {
    pub fn drawn_as(&self) -> (u32, [u32; 2]) {
        (self.slot, self.extent)
    }
}

#[derive(Debug)]
pub struct TextureBook {
    next: AtomicU64,
    revision: AtomicU64,
    moved: AtomicU64,
    listed: RwLock<FxHashMap<TextureId, Listing>>,
    minified: Mutex<Vec<TextureId>>,
}

impl Default for TextureBook {
    fn default() -> Self {
        Self {
            next: AtomicU64::new(1),
            revision: AtomicU64::new(0),
            moved: AtomicU64::new(0),
            listed: RwLock::new(FxHashMap::default()),
            minified: Mutex::new(Vec::new()),
        }
    }
}

impl TextureBook {
    pub(crate) fn allocate_id(&self) -> TextureId {
        TextureId(self.next.fetch_add(1, Ordering::Relaxed))
    }

    pub(crate) fn list(&self, id: TextureId, listing: Listing) {
        let mut listed = self.listed.write();
        listed.insert(id, listing);
        self.revision.fetch_add(1, Ordering::Release);
    }

    pub(crate) fn remove(&self, id: TextureId) -> Option<Listing> {
        let mut listed = self.listed.write();
        let gone = listed.remove(&id);
        if gone.is_some() {
            self.revision.fetch_add(1, Ordering::Release);
        }
        gone
    }

    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }

    pub fn moved(&self) -> u64 {
        self.moved.load(Ordering::Acquire)
    }

    pub fn get(&self, id: TextureId) -> Option<Listing> {
        self.listed.read().get(&id).copied()
    }

    pub(crate) fn find(&self, id: TextureId) -> Result<Listing> {
        self.get(id).ok_or(Error::Unknown(Resource::Texture(id)))
    }

    pub fn drawn_minified(&self, id: TextureId, listing: &Listing) {
        if listing.mips != Mips::Grows {
            return;
        }
        let mut listed = self.listed.write();
        let Some(listing) = listed.get_mut(&id) else {
            return;
        };
        if listing.mips != Mips::Grows {
            return;
        }
        listing.mips = Mips::Asked;
        self.minified.lock().push(id);
    }

    pub(crate) fn take_minified(&self) -> Vec<TextureId> {
        std::mem::take(&mut *self.minified.lock())
    }

    pub(crate) fn defer_request(&self, id: TextureId) {
        if self.get(id).is_some() {
            self.minified.lock().push(id);
        }
    }

    pub(crate) fn settle_mips(&self, id: TextureId, slot: Option<u32>) {
        let mut listed = self.listed.write();
        let Some(listing) = listed.get_mut(&id) else {
            return;
        };
        listing.mips = Mips::Fixed;
        let Some(slot) = slot else {
            return;
        };
        listing.slot = slot;
        let revision = self.revision.fetch_add(1, Ordering::AcqRel) + 1;
        self.moved.store(revision, Ordering::Release);
    }
}
