use std::ops::Range;

use super::{Marker, MarkerId, Markers};
use crate::clip::{Clip, ClipId};
use crate::edit::Applied;
use crate::error::{Error, Result};
use crate::project::Project;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkerScope {
    Timeline,
    Clip(ClipId),
}

#[derive(Clone, Debug, PartialEq)]
pub enum MarkerChange {
    Add(Marker),
    Set(MarkerId, Marker),
    Remove(MarkerId),
    Clear,
}

impl Clip {
    #[must_use]
    pub fn markers(&self) -> &Markers {
        &self.markers
    }

    #[must_use]
    pub fn marker_span(&self, marker: &Marker) -> Option<Range<i64>> {
        let start = self.frame_reaching(marker.at);
        if !self.covers(start) {
            return None;
        }
        let end = self.frame_reaching(marker.end()).clamp(start, self.end());
        Some(start..end)
    }
}

impl Project {
    #[must_use]
    pub fn markers(&self) -> &Markers {
        &self.markers
    }

    #[must_use]
    pub fn markers_of(&self, scope: MarkerScope) -> Option<&Markers> {
        match scope {
            MarkerScope::Timeline => Some(&self.markers),
            MarkerScope::Clip(id) => self.locate(id).map(|(_, clip)| clip.markers()),
        }
    }

    #[must_use]
    pub fn marker(&self, scope: MarkerScope, id: MarkerId) -> Option<&Marker> {
        self.markers_of(scope)?.get(id)
    }

    #[must_use]
    pub fn marker_frames(&self) -> Vec<i64> {
        let clipped = self.clips().flat_map(|(_, clip)| {
            clip.markers()
                .iter()
                .filter_map(|(_, marker)| clip.marker_span(marker))
                .map(|span| span.start)
        });
        let mut frames: Vec<i64> = self
            .markers
            .iter()
            .map(|(_, marker)| marker.at)
            .chain(clipped)
            .collect();
        frames.sort_unstable();
        frames.dedup();
        frames
    }

    pub(crate) fn mark_up(&mut self, on: MarkerScope, change: &MarkerChange) -> Result<Applied> {
        let require = |exists: bool| match exists {
            true => Ok(()),
            false => Err(Error::UnknownMarker),
        };
        match change {
            MarkerChange::Add(marker) => {
                let id = MarkerId(self.next_id());
                self.markers_mut(on)?.put(id, marker.clone().checked());
                return Ok(Applied {
                    marker: Some(id),
                    ..Applied::default()
                });
            }
            MarkerChange::Set(id, marker) => {
                let markers = self.markers_mut(on)?;
                require(markers.get(*id).is_some())?;
                markers.put(*id, marker.clone().checked());
            }
            MarkerChange::Remove(id) => require(self.markers_mut(on)?.take(*id).is_some())?,
            MarkerChange::Clear => self.markers_mut(on)?.clear(),
        }
        Ok(Applied::default())
    }

    fn markers_mut(&mut self, on: MarkerScope) -> Result<&mut Markers> {
        match on {
            MarkerScope::Timeline => Ok(&mut self.markers),
            MarkerScope::Clip(id) => {
                self.check_unlocked(&[id])?;
                Ok(&mut self.held_mut(id)?.markers)
            }
        }
    }
}
