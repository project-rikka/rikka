use cut_project::{Clip, ClipId, Edit, Marker, MarkerChange, MarkerId, MarkerScope};
use rikka_app::prelude::*;

use super::grab::frames;
use crate::timeline::scale::Scale;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Marking {
    at: Point,
    pub(crate) clip: ClipId,
    id: MarkerId,
    marker: Marker,
    from: i64,
    first: i64,
    last: i64,
    by: i64,
}

impl Marking {
    pub(crate) fn begin(at: Point, clip: &Clip, id: MarkerId) -> Option<Marking> {
        let marker = clip.markers().get(id)?.clone();
        let from = clip.marker_span(&marker)?.start;
        Some(Marking {
            at,
            clip: clip.id(),
            id,
            marker,
            from,
            first: clip.start(),
            last: clip.end() - 1,
            by: 0,
        })
    }

    pub(crate) fn drag(&mut self, to: Point, scale: &Scale) {
        self.by =
            frames(scale, to.x - self.at.x).clamp(self.first - self.from, self.last - self.from);
    }

    pub(crate) fn has_moved(&self) -> bool {
        self.by != 0
    }

    pub(crate) fn frame(&self) -> i64 {
        self.from + self.by
    }

    pub(crate) fn edit(&self, clip: &Clip) -> Edit {
        Edit::Marker {
            on: MarkerScope::Clip(self.clip),
            change: MarkerChange::Set(
                self.id,
                Marker {
                    at: clip.source_at(self.frame()),
                    ..self.marker.clone()
                },
            ),
        }
    }
}
