use cut_project::{
    Clip, ClipId, Edit, Interpolation, KeyChange, KeyRef, KeysChange, Property, Side, Smooth,
};
use rikka_app::prelude::*;

use super::{Cut, Popup};
use crate::easing::{HandleChange, Picking, Segment, held_keys};

impl Cut {
    pub(crate) fn selected_animated(&self) -> Option<&Clip> {
        self.selected_clip().filter(|clip| self.animates(clip))
    }

    fn animates(&self, clip: &Clip) -> bool {
        self.project().animates(clip)
    }

    fn animated(&self, id: ClipId) -> Option<&Clip> {
        self.project()
            .locate(id)
            .map(|(_, clip)| clip)
            .filter(|clip| self.animates(clip))
    }

    fn here(&self) -> Option<(&Clip, i64)> {
        let frame = self.transport.frame();
        self.selected_animated()
            .filter(|clip| clip.covers(frame))
            .map(|clip| (clip, frame))
    }

    pub(crate) fn toggle_key(&mut self, property: Property) {
        let edit = self.here().map(|(clip, frame)| {
            let selection = clip
                .channel(&property)
                .is_some_and(|channel| channel.has_key(clip.local(frame)));
            Edit::Key {
                clip: clip.id(),
                property,
                change: match selection {
                    true => KeyChange::Remove { at: frame },
                    false => KeyChange::Add { at: frame },
                },
            }
        });
        self.commit_edits(edit.into_iter().collect());
    }

    pub(crate) fn toggle_group(&mut self) {
        let edit = self.here().map(|(clip, frame)| {
            let here = keyed_at(clip, clip.grouped(), frame);
            match here.is_empty() {
                true => Edit::KeyGroup {
                    clip: clip.id(),
                    at: frame,
                },
                false => Edit::Keys {
                    clip: clip.id(),
                    keys: here,
                    change: KeysChange::Delete,
                },
            }
        });
        self.commit_edits(edit.into_iter().collect());
    }

    pub(crate) fn open_key_easing(&mut self, properties: Vec<Property>, anchor: Anchor) {
        self.open_easing(anchor, |_, frame| refs(properties, frame));
    }

    pub(crate) fn open_group_easing(&mut self, anchor: Anchor) {
        self.open_easing(anchor, |clip, frame| {
            match keyed_at(clip, clip.grouped(), frame) {
                keyed if keyed.is_empty() => refs(clip.grouped(), frame),
                keyed => keyed,
            }
        });
    }

    fn open_easing(&mut self, anchor: Anchor, keys: impl FnOnce(&Clip, i64) -> Vec<KeyRef>) {
        let frame = self.transport.frame();
        let Some(clip) = self.selected_animated() else {
            skip_view();
            return;
        };
        let picking = Picking::keys(clip.id(), keys(clip, frame), anchor);
        self.session.popup = Some(Popup::Easing(picking));
    }

    pub(crate) fn ease_keys(
        &mut self,
        clip: ClipId,
        keys: &[KeyRef],
        interpolation: Interpolation,
    ) {
        self.change_held(clip, keys, KeysChange::Interpolate(interpolation));
    }

    pub(crate) fn remove_keys(&mut self, clip: ClipId, keys: &[KeyRef]) {
        self.change_held(clip, keys, KeysChange::Delete);
    }

    fn change_held(&mut self, clip: ClipId, keys: &[KeyRef], change: KeysChange) {
        let edit = self
            .animated(clip)
            .map(|clip| held_keys(clip, keys))
            .filter(|keys| !keys.is_empty())
            .map(|keys| Edit::Keys { clip, keys, change });
        self.commit_edits(edit.into_iter().collect());
    }

    pub(crate) fn add_keys(&mut self, clip: ClipId, keys: &[KeyRef]) {
        let edits = keys
            .iter()
            .map(|key| Edit::Key {
                clip,
                property: key.property.clone(),
                change: KeyChange::Add { at: key.at },
            })
            .collect();
        self.commit_edits(edits);
    }

    pub(crate) fn clear_keys(&mut self, clip: ClipId, keys: &[KeyRef]) {
        let Some(animated) = self.animated(clip) else {
            skip_view();
            return;
        };
        let mut properties: Vec<Property> = keys
            .iter()
            .map(|key| key.property.clone())
            .filter(|property| animated.is_keyed(property))
            .collect();
        properties.sort();
        properties.dedup();
        let edits = properties
            .into_iter()
            .map(|property| Edit::Key {
                clip,
                property,
                change: KeyChange::Clear,
            })
            .collect();
        self.commit_edits(edits);
    }

    pub(crate) fn smooth_key(&mut self, clip: ClipId, key: &KeyRef, smooth: Smooth) {
        self.commit_edits(vec![Edit::Key {
            clip,
            property: key.property.clone(),
            change: KeyChange::Smooth { at: key.at, smooth },
        }]);
    }

    pub(crate) fn bend_segment(
        &mut self,
        clip: ClipId,
        key: &KeyRef,
        side: Side,
        direction: Point,
        done: bool,
    ) {
        let edits = self
            .animated(clip)
            .and_then(|clip| Some(Segment::for_key(clip, key)?.dragged(clip, side, direction)))
            .unwrap_or_default();
        match done {
            true => self.commit_edits(edits),
            false => self.preview_edits(&edits),
        }
    }

    pub(crate) fn set_segment_handle(
        &mut self,
        clip: ClipId,
        key: &KeyRef,
        side: Side,
        change: HandleChange,
    ) {
        let edit = self
            .animated(clip)
            .and_then(|clip| Segment::for_key(clip, key))
            .map(|segment| segment.handle_edit(clip, side, change));
        self.commit_edits(edit.into_iter().collect());
    }

    pub(crate) fn go_to_key(&mut self, later: bool) {
        let frame = self.transport.frame();
        let Some(clip) = self.selected_animated() else {
            skip_view();
            return;
        };
        let frames = clip.keyed_frames();
        let target = match later {
            true => frames.into_iter().find(|at| *at > frame),
            false => frames.into_iter().rev().find(|at| *at < frame),
        };
        match target {
            Some(at) => self.go_to(at),
            None => skip_view(),
        }
    }
}

fn refs(properties: Vec<Property>, frame: i64) -> Vec<KeyRef> {
    properties
        .into_iter()
        .map(|property| KeyRef::new(property, frame))
        .collect()
}

fn keyed_at(clip: &Clip, properties: Vec<Property>, frame: i64) -> Vec<KeyRef> {
    held_keys(clip, &refs(properties, frame))
}
