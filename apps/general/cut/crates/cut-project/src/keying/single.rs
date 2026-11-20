use super::KeyChange;
use crate::animation::Tracked;
use crate::clip::ClipId;
use crate::edit::Applied;
use crate::error::{Error, Result};
use crate::project::Project;
use crate::property::Property;

impl Project {
    pub(crate) fn key(
        &mut self,
        id: ClipId,
        property: &Property,
        change: KeyChange,
    ) -> Result<Applied> {
        let clip = self.keyable_clip_mut(id, property)?;
        let at = match change {
            KeyChange::Clear => 0,
            KeyChange::Add { at }
            | KeyChange::Remove { at }
            | KeyChange::Value { at, .. }
            | KeyChange::Handle { at, .. }
            | KeyChange::Smooth { at, .. }
            | KeyChange::Bend { at, .. } => {
                let local = at - clip.start;
                if !(0..clip.length).contains(&local) {
                    return Err(Error::OutOfReach);
                }
                local
            }
        };
        clip.keyed(property, |tracked, base| {
            let require = |done: bool| match done {
                true => Ok(()),
                false => Err(Error::NoKey),
            };
            match change {
                KeyChange::Add { .. } => {
                    if !tracked.has_key(at) {
                        let value = tracked.sample(at as f64).unwrap_or(*base);
                        tracked.set(at, value);
                    }
                    Ok(())
                }
                KeyChange::Remove { .. } => {
                    let removed = tracked.remove(at).ok_or(Error::NoKey)?;
                    if tracked.is_empty() {
                        *base = removed;
                    }
                    Ok(())
                }
                KeyChange::Clear => {
                    if let Some(first) = tracked.first() {
                        *base = first;
                        *tracked =
                            Tracked::empty_like(first, matches!(tracked, Tracked::Spatial(_)));
                    }
                    Ok(())
                }
                KeyChange::Value { value, .. } => {
                    require(tracked.has_key(at))?;
                    match tracked.set(at, value) {
                        true => Ok(()),
                        false => Err(Error::WrongValue),
                    }
                }
                KeyChange::Handle { side, handle, .. } => require(tracked.steer(at, side, handle)),
                KeyChange::Smooth { smooth, .. } => require(tracked.smooth(at, smooth)),
                KeyChange::Bend { bend, .. } => {
                    if !matches!(tracked, Tracked::Spatial(_)) {
                        return Err(Error::NotSpatial);
                    }
                    require(tracked.rebend(at, bend))
                }
            }
        })?;
        Ok(Applied::default())
    }
}
