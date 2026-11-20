use crate::keys::{Key, KeyKind};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clip {
    keys: Vec<Key>,
}

impl Clip {
    #[must_use]
    pub fn lifted(keys: Vec<Key>) -> Self {
        let earliest = keys.iter().map(Key::frame).min().unwrap_or(0);
        Self {
            keys: keys
                .into_iter()
                .map(|key| {
                    let frame = key.frame() - earliest;
                    key.with_frame(frame)
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn fits(&self, accepts: impl Fn(KeyKind) -> bool) -> bool {
        !self.keys.is_empty() && self.keys.iter().all(|key| accepts(key.kind()))
    }

    #[must_use]
    pub fn laid_at(&self, frame: u32) -> Vec<Key> {
        self.keys
            .iter()
            .cloned()
            .map(|key| {
                let frame = key.frame() + frame;
                key.with_frame(frame)
            })
            .collect()
    }
}
