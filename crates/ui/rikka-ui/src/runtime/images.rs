use rikka_renderer::TextureId;
use rustc_hash::FxHashMap;

use crate::paint::Primitive;
use crate::picture::SourceKey;

#[derive(Debug)]
struct ImageUse {
    key: SourceKey,
    prims: u32,
    shown: u32,
    waiting: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ImageHold {
    source: u32,
    placeholder: Option<u32>,
}

impl ImageHold {
    fn ids(self) -> impl Iterator<Item = u32> {
        std::iter::once(self.source).chain(self.placeholder)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Resolved {
    Source(TextureId),
    Placeholder(TextureId),
    Missing,
}

impl Resolved {
    pub(crate) fn texture(self) -> Option<TextureId> {
        match self {
            Resolved::Source(texture) | Resolved::Placeholder(texture) => Some(texture),
            Resolved::Missing => None,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Images {
    uses: Vec<Option<ImageUse>>,
    index: FxHashMap<SourceKey, u32>,
    free: Vec<u32>,
    stale: bool,
}

impl Images {
    pub(crate) fn hold(&mut self, prim: &Primitive) -> Option<ImageHold> {
        let Primitive::Image {
            source,
            placeholder,
            ..
        } = prim
        else {
            return None;
        };
        Some(ImageHold {
            source: self.intern(source.key()),
            placeholder: placeholder
                .as_ref()
                .map(|placeholder| self.intern(placeholder.key())),
        })
    }

    pub(crate) fn release(&mut self, hold: ImageHold) {
        for id in hold.ids() {
            let uses = self.entry(id);
            uses.prims -= 1;
            if uses.prims > 0 {
                continue;
            }
            if let Some(uses) = self.uses[id as usize].take() {
                self.index.remove(&uses.key);
            }
            self.free.push(id);
        }
    }

    pub(crate) fn show(&mut self, hold: ImageHold, shown: bool) {
        for id in hold.ids() {
            let uses = self.entry(id);
            match shown {
                true => uses.shown += 1,
                false => uses.shown -= 1,
            }
        }
    }

    pub(crate) fn resolved(&mut self, hold: ImageHold, resolved: Resolved) {
        self.entry(hold.source).waiting = !matches!(resolved, Resolved::Source(_));
        if let Some(placeholder) = hold.placeholder
            && !matches!(resolved, Resolved::Source(_))
        {
            self.entry(placeholder).waiting = resolved == Resolved::Missing;
        }
    }

    pub(crate) fn shown(&self) -> impl Iterator<Item = &SourceKey> {
        self.uses
            .iter()
            .flatten()
            .filter(|uses| uses.shown > 0)
            .map(|uses| &uses.key)
    }

    pub(crate) fn shows(&self, key: &SourceKey) -> bool {
        self.index.get(key).is_some_and(|&id| {
            self.uses[id as usize]
                .as_ref()
                .is_some_and(|uses| uses.shown > 0)
        })
    }

    pub(crate) fn settled(&mut self, settled_keys: &[SourceKey]) -> bool {
        let waited = settled_keys.iter().any(|key| {
            self.index.get(key).is_some_and(|&id| {
                self.uses[id as usize]
                    .as_ref()
                    .is_some_and(|uses| uses.waiting && uses.shown > 0)
            })
        });
        self.stale |= waited;
        waited
    }

    pub(crate) fn refreshed(&mut self) {
        self.stale = true;
    }

    pub(crate) fn take_stale(&mut self) -> bool {
        std::mem::take(&mut self.stale)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    fn intern(&mut self, key: SourceKey) -> u32 {
        let id = match self.index.get(&key) {
            Some(&id) => id,
            None => {
                let uses = ImageUse {
                    key: key.clone(),
                    prims: 0,
                    shown: 0,
                    waiting: false,
                };
                let id = match self.free.pop() {
                    Some(id) => {
                        self.uses[id as usize] = Some(uses);
                        id
                    }
                    None => {
                        self.uses.push(Some(uses));
                        (self.uses.len() - 1) as u32
                    }
                };
                self.index.insert(key, id);
                id
            }
        };
        self.entry(id).prims += 1;
        id
    }

    fn entry(&mut self, id: u32) -> &mut ImageUse {
        self.uses[id as usize]
            .as_mut()
            .expect("image id not interned")
    }
}
