use std::ops::Range;
use std::sync::Arc;

use crate::animation_presets::AnimationPresets;
use crate::attributes::{Attribute, KeyFit};
use crate::camera::Camera;
use crate::clip::{Clip, ClipId, Source};
use crate::effect_slot::EffectSlot;
use crate::error::{Error, Result};
use crate::footage::Treatment;
use crate::graphic::Graphic;
use crate::height::Height;
use crate::item::Item;
use crate::keying::{KeyChange, KeyPiece, KeyRef, KeysChange};
use crate::layer::Switch;
use crate::markers::{MarkerChange, MarkerId, MarkerScope};
use crate::media::Media;
use crate::piece::{Content, Piece};
use crate::project::{Edge, Format, Marks, Project};
use crate::property::Property;
use crate::region::Region;
use crate::relink::Relink;
use crate::transitions::{EditPoint, Transition};
use crate::tween::TweenChange;
use crate::visual::Appearance;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Point {
    In,
    Out,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Redraw {
    Move,
    Resize,
    Rotate,
    Style,
    Text,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    Place {
        media: Vec<Media>,
        at: i64,
        layer: usize,
    },
    Insert {
        item: Item,
        at: i64,
        length: i64,
        layer: usize,
    },
    Redraw {
        changes: Vec<(ClipId, Appearance)>,
        by: Redraw,
        at: i64,
    },
    Effects {
        clip: ClipId,
        effects: Vec<EffectSlot>,
    },
    Key {
        clip: ClipId,
        property: Property,
        change: KeyChange,
    },
    Keys {
        clip: ClipId,
        keys: Vec<KeyRef>,
        change: KeysChange,
    },
    KeyGroup {
        clip: ClipId,
        at: i64,
    },
    PasteKeys {
        clip: ClipId,
        keys: KeyPiece,
        at: i64,
    },
    AnimationPresets {
        clip: ClipId,
        animation_presets: AnimationPresets,
    },
    Tween {
        clip: ClipId,
        change: TweenChange,
    },
    Transitions {
        edit_points: Vec<EditPoint>,
        transition: Option<Transition>,
    },
    Paste {
        pieces: Vec<Piece>,
        at: i64,
        layer: usize,
    },
    PasteAttributes {
        from: Box<Piece>,
        onto: Vec<ClipId>,
        attributes: Vec<Attribute>,
        keys: KeyFit,
    },
    RemoveEffects(Vec<ClipId>),
    Relink(Vec<Relink>),
    Move {
        clips: Vec<ClipId>,
        by: i64,
        across: i32,
    },
    Trim {
        clips: Vec<ClipId>,
        edge: Edge,
        by: i64,
    },
    Slip {
        clips: Vec<ClipId>,
        by: i64,
    },
    RippleTrim {
        clip: ClipId,
        edge: Edge,
        by: i64,
    },
    Roll {
        edit_point: EditPoint,
        by: i64,
    },
    Slide {
        clip: ClipId,
        by: i64,
    },
    Extract(Range<i64>),
    Split {
        clips: Vec<ClipId>,
        at: i64,
    },
    Delete(Vec<ClipId>),
    Cut(Vec<ClipId>),
    RippleDelete(Vec<ClipId>),
    Set {
        layer: usize,
        switch: Switch,
        on: bool,
    },
    Resize {
        layer: usize,
        height: Height,
    },
    ResizeAll(Height),
    Mark {
        point: Point,
        at: i64,
    },
    ClearMarks,
    Marker {
        on: MarkerScope,
        change: MarkerChange,
    },
    Reformat(Format),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Place,
    Insert,
    Redraw(Redraw),
    Effects,
    Key,
    AnimationPresets,
    Tween,
    Transitions,
    Paste,
    PasteAttributes,
    RemoveEffects,
    Relink,
    Move,
    Trim,
    Slip,
    RippleTrim,
    Roll,
    Slide,
    Extract,
    Split,
    Delete,
    Cut,
    RippleDelete,
    Switch(Switch),
    Resize,
    Mark(Point),
    ClearMarks,
    Marker,
    Reformat,
}

impl Edit {
    #[must_use]
    pub fn action(&self) -> Action {
        match self {
            Edit::Place { .. } => Action::Place,
            Edit::Insert { .. } => Action::Insert,
            Edit::Redraw { by, .. } => Action::Redraw(*by),
            Edit::Effects { .. } => Action::Effects,
            Edit::Key { .. }
            | Edit::Keys { .. }
            | Edit::KeyGroup { .. }
            | Edit::PasteKeys { .. } => Action::Key,
            Edit::AnimationPresets { .. } => Action::AnimationPresets,
            Edit::Tween { .. } => Action::Tween,
            Edit::Transitions { .. } => Action::Transitions,
            Edit::Paste { .. } => Action::Paste,
            Edit::PasteAttributes { .. } => Action::PasteAttributes,
            Edit::RemoveEffects(_) => Action::RemoveEffects,
            Edit::Relink(_) => Action::Relink,
            Edit::Move { .. } => Action::Move,
            Edit::Trim { .. } => Action::Trim,
            Edit::Slip { .. } => Action::Slip,
            Edit::RippleTrim { .. } => Action::RippleTrim,
            Edit::Roll { .. } => Action::Roll,
            Edit::Slide { .. } => Action::Slide,
            Edit::Extract(_) => Action::Extract,
            Edit::Split { .. } => Action::Split,
            Edit::Delete(_) => Action::Delete,
            Edit::Cut(_) => Action::Cut,
            Edit::RippleDelete(_) => Action::RippleDelete,
            Edit::Set { switch, .. } => Action::Switch(*switch),
            Edit::Resize { .. } | Edit::ResizeAll(_) => Action::Resize,
            Edit::Mark { point, .. } => Action::Mark(*point),
            Edit::ClearMarks => Action::ClearMarks,
            Edit::Marker { .. } => Action::Marker,
            Edit::Reformat(_) => Action::Reformat,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Applied {
    pub selected: Option<Vec<ClipId>>,
    pub marker: Option<MarkerId>,
}

impl Applied {
    pub(crate) fn selecting(clips: Vec<ClipId>) -> Applied {
        Applied {
            selected: Some(clips),
            marker: None,
        }
    }
}

impl Project {
    pub fn apply(&self, edit: &Edit) -> Result<(Project, Applied)> {
        let mut project = self.clone();
        let applied = match edit {
            Edit::Place { media, at, layer } => project.place(media, *at, *layer)?,
            Edit::Insert {
                item,
                at,
                length,
                layer,
            } => project.insert(item, *at, *length, *layer)?,
            Edit::Redraw { changes, at, .. } => project.redraw(changes, *at)?,
            Edit::Effects { clip, effects } => project.restack(*clip, effects)?,
            Edit::Key {
                clip,
                property,
                change,
            } => project.key(*clip, property, *change)?,
            Edit::Keys { clip, keys, change } => project.rekey(*clip, keys, *change)?,
            Edit::KeyGroup { clip, at } => project.key_group(*clip, *at)?,
            Edit::PasteKeys { clip, keys, at } => project.paste_keys(*clip, keys, *at)?,
            Edit::AnimationPresets {
                clip,
                animation_presets,
            } => project.animation_preset(*clip, *animation_presets)?,
            Edit::Tween { clip, change } => project.tween(*clip, *change)?,
            Edit::Transitions {
                edit_points,
                transition,
            } => project.transit(edit_points, transition.as_ref())?,
            Edit::Paste { pieces, at, layer } => project.paste(pieces, *at, *layer)?,
            Edit::PasteAttributes {
                from,
                onto,
                attributes,
                keys,
            } => project.paste_attributes(from, onto, attributes, *keys)?,
            Edit::RemoveEffects(clips) => project.remove_effects(clips)?,
            Edit::Relink(relinks) => project.relink(relinks)?,
            Edit::Move { clips, by, across } => project.shift(clips, *by, *across)?,
            Edit::Trim { clips, edge, by } => project.trim(clips, *edge, *by)?,
            Edit::Slip { clips, by } => project.slip(clips, *by)?,
            Edit::RippleTrim { clip, edge, by } => project.ripple_trim(*clip, *edge, *by)?,
            Edit::Roll { edit_point, by } => project.roll(*edit_point, *by)?,
            Edit::Slide { clip, by } => project.slide(*clip, *by)?,
            Edit::Extract(span) => project.extract(span)?,
            Edit::Split { clips, at } => project.split(clips, *at)?,
            Edit::Delete(clips) | Edit::Cut(clips) => project.delete(clips)?,
            Edit::RippleDelete(clips) => project.ripple_delete(clips)?,
            Edit::Set { layer, switch, on } => project.set(*layer, *switch, *on),
            Edit::Resize { layer, height } => project.resize(*layer, *height),
            Edit::ResizeAll(height) => project.resize_all(*height),
            Edit::Mark { point, at } => project.mark(*point, *at),
            Edit::ClearMarks => project.clear_marks(),
            Edit::Marker { on, change } => project.mark_up(*on, change)?,
            Edit::Reformat(format) => project.reformat(*format)?,
        };
        project.normalize_transitions();
        if project.end() > Project::LONGEST {
            return Err(Error::OutOfReach);
        }
        Ok((project, applied))
    }

    fn fit(&mut self, media: &Media) {
        if let Some(picture) = media.picture()
            && !self.fitted
            && self.is_empty()
        {
            self.format = Format {
                rate: picture.rate,
                width: picture.width,
                height: picture.height,
            };
            self.fitted = true;
        }
    }

    fn place(&mut self, footage: &[Media], at: i64, layer: usize) -> Result<Applied> {
        let mut cursor = at.max(0);
        let mut selected = Vec::with_capacity(footage.len());
        for media in footage {
            self.fit(media);
            let length = media.frames(self.format.rate);
            if length < 1 {
                return Err(Error::TooShort(media.path().to_path_buf()));
            }
            let span = cursor..cursor + length;
            let target = self.free_layer(layer, &span);
            let source = Source::Footage {
                media: self.adopt(media),
                source_in: 0,
                treatment: Treatment::default(),
            };
            let clip = Clip::new(self.next_clip_id(), cursor, length, source);
            selected.push(clip.id);
            self.layer_mut(target).place(clip);
            cursor = span.end;
        }
        Ok(Applied::selecting(selected))
    }

    fn paste(&mut self, pieces: &[Piece], at: i64, layer: usize) -> Result<Applied> {
        if pieces.is_empty() {
            return Ok(Applied::default());
        }
        let at = at.max(0);
        let spans: Vec<(usize, Range<i64>)> = pieces
            .iter()
            .map(|piece| {
                (
                    piece.layer,
                    at + piece.start..at + piece.start + piece.length,
                )
            })
            .collect();
        let base = (layer..)
            .find(|base| {
                spans.iter().all(|(offset, span)| {
                    self.layer(base + offset)
                        .is_none_or(|layer_ref| layer_ref.is_free(span))
                })
            })
            .unwrap_or(layer);
        let mut selected = Vec::with_capacity(pieces.len());
        for (piece, (offset, span)) in pieces.iter().zip(spans) {
            let source = match &piece.content {
                Content::Footage {
                    media,
                    source_in,
                    treatment,
                } => Source::Footage {
                    media: self.adopt(media),
                    source_in: *source_in,
                    treatment: *treatment,
                },
                Content::Graphic(graphic) => Source::Graphic(Arc::clone(graphic)),
                Content::Region(region) => Source::Region(Arc::clone(region)),
                Content::Camera(camera) => Source::Camera(Arc::clone(camera)),
            };
            let clip = Clip {
                motion: piece.motion.clone(),
                effects: piece.effects.clone(),
                animation_presets: piece.animation_presets,
                transitions: piece.transitions,
                markers: piece.markers.clone(),
                ..Clip::new(self.next_clip_id(), span.start, piece.length, source)
            };
            selected.push(clip.id);
            self.layer_mut(base + offset).place(clip);
        }
        Ok(Applied::selecting(selected))
    }

    fn insert(&mut self, item: &Item, at: i64, length: i64, layer: usize) -> Result<Applied> {
        if length < 1 {
            return Err(Error::Empty);
        }
        let start = at.max(0);
        let span = start..start + length;
        let target = self.free_layer(layer, &span);
        let clip = Clip::new(self.next_clip_id(), start, length, item.source());
        let id = clip.id;
        self.layer_mut(target).place(clip);
        Ok(Applied::selecting(vec![id]))
    }

    fn redraw(&mut self, changes: &[(ClipId, Appearance)], at: i64) -> Result<Applied> {
        let ids: Vec<ClipId> = changes.iter().map(|(id, _)| *id).collect();
        self.check_unlocked(&ids)?;
        for (id, appearance) in changes {
            match appearance {
                Appearance::Graphic(graphic) => self.restyle(*id, graphic.clone().checked(), at)?,
                Appearance::Footage(treatment) => self.treat(*id, treatment.checked(), at)?,
                Appearance::Region(region) => self.reshape(*id, region.checked(), at)?,
                Appearance::Camera(camera) => self.reframe(*id, camera.checked(), at)?,
            }
        }
        Ok(Applied::default())
    }

    pub(crate) fn held_mut(&mut self, id: ClipId) -> Result<&mut Clip> {
        self.scope_mut(id)?.find_mut(id).ok_or(Error::UnknownClip)
    }

    fn restyle(&mut self, id: ClipId, graphic: Graphic, at: i64) -> Result<()> {
        let clip = self.held_mut(id)?;
        let Some(current) = clip.graphic().cloned() else {
            return Err(Error::NotGraphic);
        };
        let placement = clip.settled(&current.placement, &graphic.placement, at);
        let art = clip.settled(&current.art, &graphic.art, at);
        clip.source = Source::Graphic(Arc::new(Graphic { placement, art }));
        clip.prune();
        Ok(())
    }

    fn reshape(&mut self, id: ClipId, region: Region, at: i64) -> Result<()> {
        let clip = self.held_mut(id)?;
        let Some(current) = clip.region().map(|region| **region) else {
            return Err(Error::NotRegion);
        };
        let placement = clip.settled(&current.placement, &region.placement, at);
        clip.source = Source::Region(Arc::new(region.with_placement(placement)));
        Ok(())
    }

    fn reframe(&mut self, id: ClipId, camera: Camera, at: i64) -> Result<()> {
        let clip = self.held_mut(id)?;
        let Some(current) = clip.camera().map(|camera| **camera) else {
            return Err(Error::NotCamera);
        };
        let camera = clip.settled(&current, &camera, at);
        clip.source = Source::Camera(Arc::new(camera.checked()));
        Ok(())
    }

    fn treat(&mut self, id: ClipId, treatment: Treatment, at: i64) -> Result<()> {
        let (layer, clip) = self.locate(id).ok_or(Error::UnknownClip)?;
        let media = clip.media().ok_or(Error::NotFootage)?;
        let media_frames = self.media_frames(media);
        let room = self.layers[layer].next_start(id);
        let clip = self
            .layer_mut(layer)
            .find_mut(id)
            .ok_or(Error::UnknownClip)?;
        let base = clip.placement();
        let placement = clip.settled(&base, &treatment.placement, at);
        let base_mix = clip.treatment().ok_or(Error::NotFootage)?.mix;
        let mix = clip
            .settled(&base_mix, &treatment.mix, at)
            .fitted(clip.length);
        let Source::Footage {
            treatment: current, ..
        } = &mut clip.source
        else {
            return Err(Error::NotFootage);
        };
        let speed = current.speed;
        *current = Treatment {
            speed,
            placement,
            mix,
            ..treatment
        };
        if treatment.speed != speed {
            clip.stretching(|clip| clip.retime(treatment.speed, media_frames, room));
        }
        Ok(())
    }

    fn restack(&mut self, id: ClipId, effects: &[EffectSlot]) -> Result<Applied> {
        let clip = self.visual_clip_mut(id)?;
        clip.effects = effects
            .iter()
            .map(|slot| slot.clone().checked(clip.length))
            .collect();
        Ok(Applied::default())
    }

    fn animation_preset(
        &mut self,
        id: ClipId,
        animation_presets: AnimationPresets,
    ) -> Result<Applied> {
        let clip = self.visual_clip_mut(id)?;
        clip.animation_presets = animation_presets.checked();
        Ok(Applied::default())
    }

    pub(crate) fn visual_clip_mut(&mut self, id: ClipId) -> Result<&mut Clip> {
        self.check_unlocked(&[id])?;
        let (layer, clip) = self.locate(id).ok_or(Error::UnknownClip)?;
        if !self.is_visual(clip) {
            return Err(Error::NotVisual);
        }
        self.layer_mut(layer).find_mut(id).ok_or(Error::UnknownClip)
    }

    pub(crate) fn animated_clip_mut(&mut self, id: ClipId) -> Result<&mut Clip> {
        self.check_unlocked(&[id])?;
        let (layer, clip) = self.locate(id).ok_or(Error::UnknownClip)?;
        if !self.is_visual(clip) && clip.camera().is_none() {
            return Err(Error::NotVisual);
        }
        self.layer_mut(layer).find_mut(id).ok_or(Error::UnknownClip)
    }

    pub(crate) fn keyed_clip_mut(&mut self, id: ClipId) -> Result<&mut Clip> {
        self.check_unlocked(&[id])?;
        let (layer, clip) = self.locate(id).ok_or(Error::UnknownClip)?;
        if !self.animates(clip) {
            return Err(Error::NotVisual);
        }
        self.layer_mut(layer).find_mut(id).ok_or(Error::UnknownClip)
    }

    pub(crate) fn keyable_clip_mut(
        &mut self,
        id: ClipId,
        property: &Property,
    ) -> Result<&mut Clip> {
        let (_, clip) = self.locate(id).ok_or(Error::UnknownClip)?;
        if !self.keyable(clip, property) {
            return Err(Error::NotAnimatable);
        }
        self.keyed_clip_mut(id)
    }

    pub(crate) fn check_unlocked(&self, clips: &[ClipId]) -> Result<()> {
        for id in clips {
            let (layer, _) = self.locate(*id).ok_or(Error::UnknownClip)?;
            if self.layers[layer].locked {
                return Err(Error::Locked);
            }
        }
        Ok(())
    }

    fn scope_mut(&mut self, id: ClipId) -> Result<&mut crate::layer::Layer> {
        self.layers
            .iter_mut()
            .find(|layer| layer.find(id).is_some())
            .map(Arc::make_mut)
            .ok_or(Error::UnknownClip)
    }

    fn shift(&mut self, clips: &[ClipId], by: i64, across: i32) -> Result<Applied> {
        self.check_unlocked(clips)?;
        let limits = self.move_limits(clips);
        if by < limits.earliest || across < -limits.highest {
            return Err(Error::OutOfReach);
        }
        let mut landing = Vec::with_capacity(clips.len());
        for id in clips {
            let (from, _) = self.locate(*id).ok_or(Error::UnknownClip)?;
            let to = from
                .checked_add_signed(across as isize)
                .ok_or(Error::OutOfReach)?;
            if self.layer(to).is_some_and(|layer| layer.locked) {
                return Err(Error::Locked);
            }
            landing.push((from, to));
        }
        let mut lifted = Vec::with_capacity(clips.len());
        for (id, (from, to)) in clips.iter().zip(landing) {
            let mut clip = self.layer_mut(from).take(*id).ok_or(Error::UnknownClip)?;
            clip.start += by;
            lifted.push((to, clip));
        }
        for (to, clip) in &lifted {
            let mut next = self.next_id;
            self.layer_mut(*to).carve(&clip.span(), || {
                next += 1;
                ClipId(next - 1)
            });
            self.next_id = next;
        }
        for (to, clip) in lifted {
            self.layer_mut(to).place(clip);
        }
        Ok(Applied::selecting(clips.to_vec()))
    }

    fn trim(&mut self, clips: &[ClipId], edge: Edge, by: i64) -> Result<Applied> {
        self.check_unlocked(clips)?;
        if !self.trim_limits(clips, edge).contains(&by) {
            return Err(Error::OutOfReach);
        }
        for id in clips {
            let clip = self
                .scope_mut(*id)?
                .find_mut(*id)
                .ok_or(Error::UnknownClip)?;
            clip.stretching(|clip| match edge {
                Edge::Head => clip.cut_head(by),
                Edge::Tail => clip.cut_tail(-by),
            });
        }
        Ok(Applied::selecting(clips.to_vec()))
    }

    fn slip(&mut self, clips: &[ClipId], by: i64) -> Result<Applied> {
        self.check_unlocked(clips)?;
        if !self.slip_limits(clips).contains(&by) {
            return Err(Error::OutOfReach);
        }
        for id in clips {
            let clip = self
                .scope_mut(*id)?
                .find_mut(*id)
                .ok_or(Error::UnknownClip)?;
            clip.slip(by);
        }
        Ok(Applied::selecting(clips.to_vec()))
    }

    fn split(&mut self, clips: &[ClipId], at: i64) -> Result<Applied> {
        self.check_unlocked(clips)?;
        let mut made = 0;
        for id in clips {
            let fresh = self.next_clip_id();
            let layer = self.scope_mut(*id)?;
            let clip = layer.find_mut(*id).ok_or(Error::UnknownClip)?;
            if at <= clip.start || at >= clip.end() {
                continue;
            }
            let right = clip.divided(fresh, at);
            layer.place(right);
            made += 1;
        }
        if made == 0 {
            return Err(Error::NothingToSplit);
        }
        Ok(Applied::default())
    }

    fn delete(&mut self, clips: &[ClipId]) -> Result<Applied> {
        self.lift(clips)?;
        Ok(Applied::selecting(Vec::new()))
    }

    fn lift(&mut self, clips: &[ClipId]) -> Result<Vec<Range<i64>>> {
        self.check_unlocked(clips)?;
        let mut spans = Vec::with_capacity(clips.len());
        for id in clips {
            let clip = self.scope_mut(*id)?.take(*id).ok_or(Error::UnknownClip)?;
            spans.push(clip.span());
        }
        Ok(spans)
    }

    fn ripple_delete(&mut self, clips: &[ClipId]) -> Result<Applied> {
        let gaps = merged(self.lift(clips)?);
        for gap in gaps.iter().rev() {
            let open = self
                .layers
                .iter()
                .filter(|layer| !layer.locked)
                .all(|layer| layer.is_empty_over(gap));
            if !open {
                continue;
            }
            self.ripple_from(gap.end, gap.start - gap.end, None);
        }
        Ok(Applied::selecting(Vec::new()))
    }

    fn set(&mut self, layer: usize, switch: Switch, on: bool) -> Applied {
        self.layer_mut(layer).set(switch, on);
        Applied::default()
    }

    fn resize(&mut self, layer: usize, height: Height) -> Applied {
        if self.height_of(layer) != height {
            self.layer_mut(layer).height = Some(height);
        }
        Applied::default()
    }

    fn resize_all(&mut self, height: Height) -> Applied {
        self.layer_height = height;
        for layer in self
            .layers
            .iter_mut()
            .filter(|layer| layer.height.is_some())
        {
            Arc::make_mut(layer).height = None;
        }
        Applied::default()
    }

    fn mark(&mut self, point: Point, at: i64) -> Applied {
        let at = at.max(0);
        match point {
            Point::In => {
                self.marks.mark_in = Some(at);
                if self.marks.mark_out.is_some_and(|out| out < at) {
                    self.marks.mark_out = None;
                }
            }
            Point::Out => {
                self.marks.mark_out = Some(at);
                if self.marks.mark_in.is_some_and(|mark| mark > at) {
                    self.marks.mark_in = None;
                }
            }
        }
        Applied::default()
    }

    fn clear_marks(&mut self) -> Applied {
        self.marks = Marks::default();
        Applied::default()
    }
}

fn merged(mut spans: Vec<Range<i64>>) -> Vec<Range<i64>> {
    spans.sort_by_key(|span| span.start);
    let mut gaps: Vec<Range<i64>> = Vec::with_capacity(spans.len());
    for span in spans {
        match gaps.last_mut() {
            Some(last) if span.start <= last.end => last.end = last.end.max(span.end),
            _ => gaps.push(span),
        }
    }
    gaps
}
