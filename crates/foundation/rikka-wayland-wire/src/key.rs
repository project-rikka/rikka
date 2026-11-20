use std::any::TypeId;

use smallvec::SmallVec;

pub trait Key: Copy + 'static {
    fn into_raw(self) -> u64;

    fn from_raw(raw: u64) -> Self;
}

impl Key for u32 {
    fn into_raw(self) -> u64 {
        u64::from(self)
    }

    fn from_raw(raw: u64) -> Self {
        raw as u32
    }
}

impl Key for u64 {
    fn into_raw(self) -> u64 {
        self
    }

    fn from_raw(raw: u64) -> Self {
        raw
    }
}

#[macro_export]
macro_rules! slot_keys {
    ($($key:ty),+ $(,)?) => {$(
        impl $crate::Key for $key {
            fn into_raw(self) -> u64 {
                $crate::__slotmap::Key::data(&self).as_ffi()
            }

            fn from_raw(raw: u64) -> Self {
                $crate::__slotmap::KeyData::from_ffi(raw).into()
            }
        }
    )+};
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Keys(SmallVec<[(TypeId, u64); 2]>);

impl Keys {
    pub(crate) fn set<K: Key>(&mut self, key: K) {
        let kind = TypeId::of::<K>();
        let raw = key.into_raw();
        match self.0.iter_mut().find(|(stored, _)| *stored == kind) {
            Some(entry) => entry.1 = raw,
            None => self.0.push((kind, raw)),
        }
    }

    pub(crate) fn get<K: Key>(&self) -> Option<K> {
        let kind = TypeId::of::<K>();
        self.0
            .iter()
            .find(|(stored, _)| *stored == kind)
            .map(|(_, raw)| K::from_raw(*raw))
    }

    pub(crate) fn take<K: Key>(&mut self) -> Option<K> {
        let kind = TypeId::of::<K>();
        let index = self.0.iter().position(|(stored, _)| *stored == kind)?;
        Some(K::from_raw(self.0.swap_remove(index).1))
    }
}
