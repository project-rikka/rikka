use cut_project::{ClipId, Edit, Marker, MarkerChange, MarkerId, MarkerScope};
use rikka_app::prelude::*;

use super::Cut;

#[derive(Clone, Debug)]
pub(crate) struct Marking {
    pub(crate) scope: MarkerScope,
    pub(crate) id: MarkerId,
    pub(crate) name: TextDocument,
    pub(crate) note: TextDocument,
    committed: (String, String),
}

impl Marking {
    fn new(scope: MarkerScope, id: MarkerId, marker: &Marker) -> Marking {
        Marking {
            scope,
            id,
            name: TextDocument::new(&marker.name),
            note: TextDocument::new(&marker.note),
            committed: (marker.name.clone(), marker.note.clone()),
        }
    }
}

impl Cut {
    pub(crate) fn marking(&self) -> Option<&Marking> {
        self.session.marking.as_ref()
    }

    pub(crate) fn add_marker(&mut self) {
        let frame = self.transport.frame();
        let existing_marker = self
            .project()
            .markers()
            .iter()
            .find(|(_, marker)| marker.at == frame)
            .map(|(id, _)| id);
        if let Some(id) = existing_marker {
            self.open_marker(MarkerScope::Timeline, id);
            return;
        }
        self.mark_up(MarkerScope::Timeline, Marker::new(frame));
    }

    pub(crate) fn add_clip_marker(&mut self) {
        let frame = self.transport.frame();
        let Some(clip) = self.marked_clip(frame) else {
            skip_view();
            return;
        };
        let Some((_, clip_ref)) = self.project().locate(clip) else {
            return;
        };
        let at = clip_ref.source_at(frame);
        self.mark_up(MarkerScope::Clip(clip), Marker::new(at));
    }

    pub(crate) fn can_add_clip_marker(&self) -> bool {
        self.marked_clip(self.transport.frame()).is_some()
    }

    fn marked_clip(&self, frame: i64) -> Option<ClipId> {
        let under = self.project().clips_under(frame);
        under
            .iter()
            .rev()
            .find(|id| self.session.selected.contains(id))
            .or_else(|| under.last())
            .copied()
    }

    fn mark_up(&mut self, scope: MarkerScope, marker: Marker) {
        self.commit_marker();
        let applied = self.edit_applied(Edit::Marker {
            on: scope,
            change: MarkerChange::Add(marker),
        });
        if let Some(id) = applied.and_then(|applied| applied.marker) {
            self.open_marker(scope, id);
        }
    }

    pub(crate) fn marker_frames(&self) -> Vec<i64> {
        self.project().marker_frames()
    }

    pub(crate) fn go_to_marker(&mut self, later: bool) {
        let frame = self.transport.frame();
        let frames = self.marker_frames();
        let marker_frame = match later {
            true => frames.into_iter().find(|at| *at > frame),
            false => frames.into_iter().rev().find(|at| *at < frame),
        };
        match marker_frame {
            Some(marker_frame) => self.go_to(marker_frame),
            None => skip_view(),
        }
    }

    pub(crate) fn marker_here(&self) -> Option<(MarkerScope, MarkerId)> {
        let frame = self.transport.frame();
        let project = self.project();
        if let Some((id, _)) = project
            .markers()
            .iter()
            .find(|(_, marker)| (marker.at..marker.end().max(marker.at + 1)).contains(&frame))
        {
            return Some((MarkerScope::Timeline, id));
        }
        self.session
            .selected
            .iter()
            .filter_map(|id| project.locate(*id))
            .find_map(|(_, clip)| {
                clip.markers()
                    .iter()
                    .find(|(_, marker)| {
                        clip.marker_span(marker)
                            .is_some_and(|span| span.start == frame || span.contains(&frame))
                    })
                    .map(|(id, _)| (MarkerScope::Clip(clip.id()), id))
            })
    }

    pub(crate) fn edit_marker_here(&mut self) {
        match self.marker_here() {
            Some((scope, id)) => self.open_marker(scope, id),
            None => skip_view(),
        }
    }

    pub(crate) fn delete_marker_here(&mut self) {
        if let Some((scope, id)) = self.marker_here() {
            self.remove_marker(scope, id);
        }
    }

    pub(crate) fn clear_markers(&mut self) {
        self.commit_marker();
        self.edit(Edit::Marker {
            on: MarkerScope::Timeline,
            change: MarkerChange::Clear,
        });
    }

    pub(crate) fn open_marker(&mut self, scope: MarkerScope, id: MarkerId) {
        self.commit_marker();
        let Some(marker) = self.project().marker(scope, id) else {
            self.session.marking = None;
            return;
        };
        let marking = Marking::new(scope, id, marker);
        self.session.popup = None;
        self.session.marking = Some(marking);
    }

    pub(crate) fn close_marker(&mut self) {
        self.commit_marker();
        self.session.marking = None;
    }

    pub(crate) fn commit_marker(&mut self) {
        let Some(marking) = &self.session.marking else {
            return;
        };
        let (name, note) = (marking.name.text(), marking.note.text());
        self.set_marker(|marker| {
            marker.name = name;
            marker.note = note;
        });
    }

    pub(crate) fn change_marker(&mut self, change: impl FnOnce(&mut Marker)) {
        if !self.set_marker(change) {
            skip_view();
        }
    }

    fn set_marker(&mut self, change: impl FnOnce(&mut Marker)) -> bool {
        let Some((scope, id)) = self
            .session
            .marking
            .as_ref()
            .map(|marking| (marking.scope, marking.id))
        else {
            return false;
        };
        let Some(marker_ref) = self.project().marker(scope, id) else {
            return false;
        };
        let mut marker = marker_ref.clone();
        change(&mut marker);
        if marker == *marker_ref {
            return false;
        }
        self.edit(Edit::Marker {
            on: scope,
            change: MarkerChange::Set(id, marker),
        });
        true
    }

    pub(crate) fn remove_marker(&mut self, scope: MarkerScope, id: MarkerId) {
        self.edit(Edit::Marker {
            on: scope,
            change: MarkerChange::Remove(id),
        });
    }

    pub(super) fn sync_marking(&mut self) {
        let Some(marking) = &mut self.session.marking else {
            return;
        };
        let Some(marker) = self.document.project().marker(marking.scope, marking.id) else {
            self.session.marking = None;
            return;
        };
        if marker.name != marking.committed.0 {
            marking.name.set_text(&marker.name);
            marking.committed.0.clone_from(&marker.name);
        }
        if marker.note != marking.committed.1 {
            marking.note.set_text(&marker.note);
            marking.committed.1.clone_from(&marker.note);
        }
    }
}
