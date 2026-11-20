use super::KeyRef;
use crate::animation::AnyKey;
use crate::clip::ClipId;
use crate::edit::Applied;
use crate::error::{Error, Result};
use crate::project::Project;
use crate::property::Property;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeyPiece {
    keys: Vec<(Property, AnyKey)>,
}

impl KeyPiece {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    #[must_use]
    pub fn keys(&self) -> Vec<KeyRef> {
        self.keys
            .iter()
            .map(|(property, key)| KeyRef::new(property.clone(), key.at()))
            .collect()
    }
}

impl Project {
    #[must_use]
    pub fn copy_keys(&self, id: ClipId, keys: &[KeyRef]) -> KeyPiece {
        let Some((_, clip)) = self.locate(id) else {
            return KeyPiece::default();
        };
        let mut copied: Vec<(Property, AnyKey)> = keys
            .iter()
            .filter_map(|key| {
                let any_key = clip.channel(&key.property)?.any_key(key.at - clip.start)?;
                Some((key.property.clone(), any_key))
            })
            .collect();
        let earliest = copied.iter().map(|(_, key)| key.at()).min().unwrap_or(0);
        for (_, key) in &mut copied {
            *key = key.moved(key.at() - earliest);
        }
        copied.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.at().cmp(&right.1.at())));
        KeyPiece { keys: copied }
    }

    pub(crate) fn paste_keys(&mut self, id: ClipId, piece: &KeyPiece, at: i64) -> Result<Applied> {
        let (_, clip) = self.locate(id).ok_or(Error::UnknownClip)?;
        let placed: Vec<bool> = piece
            .keys
            .iter()
            .map(|(property, _)| self.keyable(clip, property))
            .collect();
        let clip = self.keyed_clip_mut(id)?;
        let offset = at - clip.start;
        let length = clip.length;
        for ((property, key), keyable) in piece.keys.iter().zip(placed) {
            let landing = offset + key.at();
            if !(0..length).contains(&landing) || !keyable {
                continue;
            }
            clip.keyed(property, |tracked, _| {
                tracked.put(key.moved(landing));
                Ok(())
            })?;
        }
        Ok(Applied::default())
    }
}
