use std::collections::BTreeMap;

use super::{KeyRef, KeysChange};
use crate::animation::AnyKey;
use crate::clip::ClipId;
use crate::edit::Applied;
use crate::error::{Error, Result};
use crate::project::Project;
use crate::property::Property;

impl Project {
    pub(crate) fn rekey(
        &mut self,
        id: ClipId,
        keys: &[KeyRef],
        change: KeysChange,
    ) -> Result<Applied> {
        let clip = self.keyed_clip_mut(id)?;
        let mut selected: BTreeMap<Property, Vec<i64>> = BTreeMap::new();
        for key in keys {
            if !clip.holds(key) {
                return Err(Error::NoKey);
            }
            selected
                .entry(key.property.clone())
                .or_default()
                .push(key.at - clip.start);
        }
        let Some(lowest) = selected.values().flatten().min().copied() else {
            return Ok(Applied::default());
        };
        let highest = selected.values().flatten().max().copied().unwrap_or(lowest);
        let reach = -lowest..=clip.length - 1 - highest;
        for (property, frames) in &selected {
            clip.keyed(property, |tracked, base| {
                match change {
                    KeysChange::Delete => {
                        let mut cut = tracked.take(frames);
                        cut.sort_by_key(|key| key.at());
                        if tracked.is_empty()
                            && let Some(first) = cut.first()
                        {
                            *base = first.value();
                        }
                    }
                    KeysChange::Move { by } | KeysChange::Duplicate { by } => {
                        let by = by.clamp(*reach.start(), *reach.end());
                        if by == 0 {
                            return Ok(());
                        }
                        let moved: Vec<AnyKey> = match change {
                            KeysChange::Duplicate { .. } => {
                                frames.iter().filter_map(|at| tracked.key(*at)).collect()
                            }
                            _ => tracked.take(frames),
                        };
                        for key in moved {
                            tracked.put(key.moved(key.at() + by));
                        }
                    }
                    KeysChange::Interpolate(interpolation) => {
                        for at in frames {
                            tracked.interpolate(*at, interpolation);
                        }
                    }
                }
                Ok(())
            })?;
        }
        Ok(Applied::default())
    }

    pub(crate) fn key_group(&mut self, id: ClipId, at: i64) -> Result<Applied> {
        let (_, clip) = self.locate(id).ok_or(Error::UnknownClip)?;
        if !clip
            .grouped()
            .iter()
            .all(|property| self.keyable(clip, property))
        {
            return Err(Error::NotAnimatable);
        }
        let clip = self.animated_clip_mut(id)?;
        let local = at - clip.start;
        if !(0..clip.length).contains(&local) {
            return Err(Error::OutOfReach);
        }
        for property in clip.grouped() {
            clip.keyed(&property, |tracked, base| {
                if !tracked.has_key(local) {
                    let value = tracked.sample(local as f64).unwrap_or(*base);
                    tracked.set(local, value);
                }
                Ok(())
            })?;
        }
        Ok(Applied::default())
    }
}
