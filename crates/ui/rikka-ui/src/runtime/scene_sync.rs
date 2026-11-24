use rikka_renderer::{
    Blur, GroupId, ImageSource, LayerId, Node, NodeId, Scene, Stroke, TextureId, Transform,
};
use rikka_types::{Color, CornerRadii, Point, Rect, RoundedRect};

use crate::paint::{ClipKind, PaintedPrimitive, Primitive, SceneClip};
use crate::picture::TextureResolver;
use crate::runtime::images::{ImageHold, Images, Resolved};
use crate::runtime::layout::Node as LayoutNode;
use crate::runtime::scratch::pooled;
use crate::runtime::table::Labels;
use rikka_picture::{Changes, Repaint};

pooled! {
    PrimSlot => PRIM_SLOTS,
}

const TOPMOST: i32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Frame {
    pub(crate) group: Option<GroupId>,
    pub(crate) origin: Point,
    pub(crate) exact: Point,
}

impl Frame {
    pub(crate) fn at(group: GroupId, exact: Point) -> Self {
        Frame {
            group: Some(group),
            origin: Point::new(exact.x.round(), exact.y.round()),
            exact,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GroupScroll {
    pub(crate) group: GroupId,
    pub(crate) by: Point,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Placement {
    pub(crate) group: Option<GroupId>,
    pub(crate) z: i32,
    pub(crate) stack: u64,
    pub(crate) label: u64,
    pub(crate) layer: Option<LayerId>,
    pub(crate) clip_group: Option<GroupId>,
    pub(crate) bounded: bool,
    pub(crate) visible: bool,
}

pub(crate) struct PrimSlot {
    pub(crate) prim: Primitive,
    pub(crate) node: NodeId,
    pub(crate) resolved: Resolved,
    pub(crate) placement: Placement,
    pub(crate) origin: Point,
    pub(crate) clip_origin: Point,
    pub(crate) fade: Option<usize>,
    pub(crate) clip: ClipKind,
    pub(crate) shown_clip: Option<RoundedRect>,
    pub(crate) scene: SceneClip,
    pub(crate) image: Option<ImageHold>,
    pub(crate) topmost: bool,
    pub(crate) section: Option<u32>,
    pub(crate) part: Option<u64>,
    pub(crate) version: u64,
}

impl PrimSlot {
    pub(crate) fn painted(&self) -> Primitive {
        self.prim
            .clone()
            .unplaced(self.origin, self.clip_origin)
            .with_clip(self.shown_clip)
    }

    pub(crate) fn repainted(
        &self,
        shift: Point,
        base: Option<RoundedRect>,
        loose: Option<RoundedRect>,
    ) -> PaintedPrimitive {
        let prim = self.painted().shifted(shift);
        let prim = match self.clip {
            ClipKind::Base => prim.with_clip(base),
            ClipKind::Local | ClipKind::Volatile => prim,
        };
        let scene = match self.scene.kind {
            ClipKind::Base => SceneClip::base(loose),
            kind => SceneClip {
                shape: self
                    .scene
                    .shape
                    .map(|shape| RoundedRect::new(shape.rect.translate(shift), shape.radii)),
                kind,
                section: self.scene.section,
            },
        };
        PaintedPrimitive {
            prim,
            fade: self.fade,
            clip: self.clip,
            scene,
            topmost: self.topmost,
            section: self.section,
            part: self.part,
        }
    }
}

pub(crate) struct Sink<'a> {
    pub(crate) scene: &'a mut Scene,
    pub(crate) resources: &'a mut dyn TextureResolver,
    pub(crate) images: &'a mut Images,
}

#[derive(Clone, Copy)]
pub(crate) struct Placing<'a> {
    pub(crate) frame: Frame,
    pub(crate) layer: Option<LayerId>,
    pub(crate) fades: &'a [LayerId],
    pub(crate) sections: &'a [GroupScroll],
    pub(crate) stack: u64,
    pub(crate) labels: &'a Labels,
    pub(crate) node: LayoutNode,
    pub(crate) base: Frame,
    pub(crate) bounded: bool,
}

impl Placing<'_> {
    fn section(&self, section: Option<u32>) -> Frame {
        match section.and_then(|at| self.sections.get(at as usize)) {
            Some(scrolled) => Frame {
                group: Some(scrolled.group),
                origin: self.frame.origin + scrolled.by,
                exact: self.frame.exact + scrolled.by,
            },
            None => self.frame,
        }
    }

    fn clip_frame(&self, scene: SceneClip) -> Frame {
        match scene.kind {
            _ if scene.shape.is_none() => self.frame,
            ClipKind::Base => self.base,
            ClipKind::Local => self.section(scene.section),
            ClipKind::Volatile => Frame::default(),
        }
    }

    fn placement(&self, index: usize, painted: &PaintedPrimitive, clip: Frame) -> Placement {
        let topmost = painted.topmost;
        Placement {
            group: self.section(painted.section).group,
            z: match topmost {
                true => TOPMOST,
                false => 0,
            },
            stack: self.stack,
            label: self.labels.prim(self.node, index),
            layer: painted
                .fade
                .and_then(|fade| self.fades.get(fade).copied())
                .or(self.layer),
            clip_group: clip.group,
            bounded: self.bounded && !topmost,
            visible: true,
        }
    }
}

#[derive(Default)]
pub(crate) struct Regrouping {
    old: Vec<(Option<u64>, u32)>,
    new: Vec<(Option<u64>, u32)>,
    removed_slots: Vec<Option<PrimSlot>>,
    arranged: Vec<Option<PrimSlot>>,
}

impl Regrouping {
    fn arrange(
        &mut self,
        sink: &mut Sink<'_>,
        slots: &mut Vec<PrimSlot>,
        new: &[PaintedPrimitive],
    ) {
        self.old.clear();
        self.old.extend(
            slots
                .iter()
                .enumerate()
                .map(|(at, slot)| (slot.part, at as u32)),
        );
        self.old.sort_unstable();
        self.new.clear();
        self.new.extend(
            new.iter()
                .enumerate()
                .map(|(at, painted)| (painted.part, at as u32)),
        );
        self.new.sort_unstable();
        self.removed_slots.clear();
        self.removed_slots.extend(slots.drain(..).map(Some));
        self.arranged.clear();
        self.arranged.resize_with(new.len(), || None);

        let (mut was, mut now) = (0, 0);
        while let (Some(&(part, from)), Some(&(target_part, to))) =
            (self.old.get(was), self.new.get(now))
        {
            match part.cmp(&target_part) {
                std::cmp::Ordering::Less => was += 1,
                std::cmp::Ordering::Greater => now += 1,
                std::cmp::Ordering::Equal => {
                    self.arranged[to as usize] = self.removed_slots[from as usize].take();
                    was += 1;
                    now += 1;
                }
            }
        }
        for (at, place) in self.arranged.iter_mut().enumerate() {
            if place.is_some() {
                continue;
            }
            let kind = std::mem::discriminant(&new[at].prim);
            *place = self
                .removed_slots
                .iter_mut()
                .find(|left| {
                    left.as_ref()
                        .is_some_and(|slot| std::mem::discriminant(&slot.prim) == kind)
                })
                .and_then(Option::take);
        }
        for left in self.removed_slots.drain(..).flatten() {
            forget(sink.scene, sink.images, left);
        }
    }
}

fn parts_moved(slots: &[PrimSlot], new: &[PaintedPrimitive]) -> bool {
    slots
        .iter()
        .zip(new)
        .any(|(slot, painted)| slot.part != painted.part)
}

pub(crate) fn apply_prims(
    sink: &mut Sink<'_>,
    slots: &mut Vec<PrimSlot>,
    new: &mut Vec<PaintedPrimitive>,
    placing: Placing<'_>,
    regrouping: &mut Regrouping,
) {
    if parts_moved(slots, new) {
        regrouping.arrange(sink, slots, new);
        for (index, (painted, previous_slot)) in new
            .iter_mut()
            .zip(regrouping.arranged.drain(..))
            .enumerate()
        {
            match previous_slot {
                Some(mut slot) => {
                    settle(sink, &mut slot, painted, index, placing);
                    slots.push(slot);
                }
                None => slots.push(fresh(sink, painted, index, placing)),
            }
        }
        new.clear();
        return;
    }
    if new.len() < slots.len() {
        for slot in slots.drain(new.len()..) {
            forget(sink.scene, sink.images, slot);
        }
    }
    for (index, painted) in new.iter_mut().enumerate() {
        match slots.get_mut(index) {
            Some(slot) => settle(sink, slot, painted, index, placing),
            None => slots.push(fresh(sink, painted, index, placing)),
        }
    }
    new.clear();
}

struct Prepared {
    placement: Placement,
    resolved: Resolved,
    image: Option<ImageHold>,
    origin: Point,
    clip_origin: Point,
    shown_clip: Option<RoundedRect>,
}

#[inline(always)]
fn prepare(
    sink: &mut Sink<'_>,
    painted: &mut PaintedPrimitive,
    index: usize,
    placing: Placing<'_>,
) -> Prepared {
    let clip_frame = placing.clip_frame(painted.scene);
    let placement = placing.placement(index, painted, clip_frame);
    let prim = &mut painted.prim;
    let shown_clip = prim.clip();
    prim.set_clip(painted.scene.shape);
    prim.place(placing.frame.origin, clip_frame.origin);
    let image = sink.images.hold(prim);
    let resolved = resolve(prim, sink.resources);
    if let Some(image) = image {
        sink.images.resolved(image, resolved);
        sink.images.show(image, true);
    }
    Prepared {
        placement,
        resolved,
        image,
        origin: placing.frame.origin,
        clip_origin: clip_frame.origin,
        shown_clip,
    }
}

fn fresh(
    sink: &mut Sink<'_>,
    painted: &mut PaintedPrimitive,
    index: usize,
    placing: Placing<'_>,
) -> PrimSlot {
    let prepared = prepare(sink, painted, index, placing);
    let node = insert_prim(
        sink.scene,
        &painted.prim,
        prepared.resolved.texture(),
        prepared.placement,
    );
    let version = latest(&painted.prim, sink.resources);
    let prim = std::mem::replace(
        &mut painted.prim,
        Primitive::Segment {
            rect: Rect::ZERO,
            rotation: 0.0,
            color: Color::TRANSPARENT,
            clip: None,
        },
    );
    PrimSlot {
        prim,
        node,
        resolved: prepared.resolved,
        placement: prepared.placement,
        origin: prepared.origin,
        clip_origin: prepared.clip_origin,
        fade: painted.fade,
        clip: painted.clip,
        shown_clip: prepared.shown_clip,
        scene: painted.scene,
        image: prepared.image,
        topmost: painted.topmost,
        section: painted.section,
        part: painted.part,
        version,
    }
}

fn settle(
    sink: &mut Sink<'_>,
    slot: &mut PrimSlot,
    painted: &mut PaintedPrimitive,
    index: usize,
    placing: Placing<'_>,
) {
    slot.fade = painted.fade;
    slot.clip = painted.clip;
    slot.scene = painted.scene;
    slot.topmost = painted.topmost;
    slot.section = painted.section;
    slot.part = painted.part;
    let Prepared {
        placement,
        resolved,
        image,
        origin,
        clip_origin,
        shown_clip,
    } = prepare(sink, painted, index, placing);
    if let Some(previous_slot) = slot.image.take() {
        if slot.placement.visible {
            sink.images.show(previous_slot, false);
        }
        sink.images.release(previous_slot);
    }
    slot.image = image;
    slot.shown_clip = shown_clip;
    slot.origin = origin;
    slot.clip_origin = clip_origin;
    let prim = &mut painted.prim;
    if slot.prim == *prim && slot.resolved == resolved && slot.placement == placement {
        return;
    }
    let reusable = slot.placement.group == placement.group
        && update_in_place(sink.scene, slot, prim, resolved.texture());
    if !reusable || slot.resolved != resolved {
        slot.version = latest(prim, sink.resources);
    }
    if !reusable {
        sink.scene.remove(slot.node);
        slot.node = insert_prim(sink.scene, prim, resolved.texture(), placement);
    } else if slot.placement != placement {
        sink.scene
            .edit(slot.node)
            .z(placement.z)
            .stack(placement.stack)
            .layer(placement.layer)
            .order(placement.label)
            .clip_in(placement.clip_group)
            .clipped_by_group(placement.bounded)
            .visible(placement.visible);
    }
    slot.placement = placement;
    std::mem::swap(&mut slot.prim, prim);
    slot.resolved = resolved;
}

pub(crate) fn show_prims(sink: &mut Sink<'_>, slots: &mut [PrimSlot], visible: bool) {
    for slot in slots {
        if slot.placement.visible == visible {
            continue;
        }
        slot.placement.visible = visible;
        sink.scene.edit(slot.node).visible(visible);
        if let Some(image) = slot.image {
            sink.images.show(image, visible);
        }
    }
}

pub(crate) fn relabel_prims(
    scene: &mut Scene,
    slots: &mut [PrimSlot],
    labels: &Labels,
    node: LayoutNode,
) {
    for (index, slot) in slots.iter_mut().enumerate() {
        let label = labels.prim(node, index);
        if slot.placement.label != label {
            slot.placement.label = label;
            scene.edit(slot.node).order(label);
        }
    }
}

pub(crate) fn forget_prims(scene: &mut Scene, images: &mut Images, slots: &mut Vec<PrimSlot>) {
    for slot in slots.drain(..) {
        forget(scene, images, slot);
    }
}

fn forget(scene: &mut Scene, images: &mut Images, slot: PrimSlot) {
    scene.remove(slot.node);
    if let Some(image) = slot.image {
        if slot.placement.visible {
            images.show(image, false);
        }
        images.release(image);
    }
}

fn latest(prim: &Primitive, resources: &mut dyn TextureResolver) -> u64 {
    changes(prim, u64::MAX, resources).serial
}

fn changes(prim: &Primitive, version: u64, resources: &mut dyn TextureResolver) -> Changes {
    match prim {
        Primitive::Image { source, .. } => resources.changes(source, version),
        _ => Changes::UNKNOWN,
    }
}

fn repainted_areas<'a>(
    prim: &Primitive,
    repaint: &'a Repaint,
) -> Option<impl Iterator<Item = Rect> + 'a> {
    let Repaint::Within(parts) = repaint else {
        return None;
    };
    let Primitive::Image {
        rect, turn, mirror, ..
    } = prim
    else {
        return None;
    };
    if *turn != 0.0 || *mirror != Transform::Normal {
        return None;
    }
    let rect = *rect;
    Some(parts.iter().map(move |part| {
        Rect::new(
            rect.x() + part.x() * rect.width() - 1.0,
            rect.y() + part.y() * rect.height() - 1.0,
            part.width() * rect.width() + 2.0,
            part.height() * rect.height() + 2.0,
        )
    }))
}

fn resolve(prim: &Primitive, resources: &mut dyn TextureResolver) -> Resolved {
    let Primitive::Image {
        source,
        placeholder,
        ..
    } = prim
    else {
        return Resolved::Missing;
    };
    if let Some(texture) = resources.resolve(source) {
        return Resolved::Source(texture);
    }
    match placeholder
        .as_ref()
        .and_then(|placeholder| resources.resolve(placeholder))
    {
        Some(texture) => Resolved::Placeholder(texture),
        None => Resolved::Missing,
    }
}

fn insert_prim(
    scene: &mut Scene,
    prim: &Primitive,
    resolved: Option<TextureId>,
    placement: Placement,
) -> NodeId {
    let mut node = build_node(prim, placement.z, resolved)
        .stack(placement.stack)
        .order(placement.label)
        .clipped_by_group(placement.bounded)
        .visible(placement.visible);
    if let Some(group) = placement.clip_group {
        node = node.clip_in(group);
    }
    if let Some(group) = placement.group {
        node = node.group(group);
    }
    if let Some(layer) = placement.layer {
        node = node.layer(layer);
    }
    scene.insert(node)
}

fn build_node(prim: &Primitive, z: i32, resolved: Option<TextureId>) -> Node {
    match prim {
        Primitive::Quad {
            rect,
            fill,
            fill_gradient,
            radii,
            stroke,
            stroke_gradient,
            blur,
            lens,
            cutout,
            clip,
            opacity,
        } => {
            let mut node = Node::rounded_rect(*rect, *radii)
                .fill(*fill)
                .opacity(*opacity)
                .z(z);
            if let Some(gradient) = fill_gradient {
                node = node.fill_gradient(**gradient);
            }
            if let Some((width, color)) = stroke {
                node = node.stroke(Stroke::new(*width, *color));
            }
            if let Some(gradient) = stroke_gradient {
                node = node.stroke_gradient(**gradient);
            }
            if let Some(sigma) = blur {
                node = node.backdrop(Blur::new(*sigma));
            }
            if let Some(lens) = lens {
                node = node.lens(*lens);
            }
            if let Some(cutout) = cutout {
                node = node.cut_out(*cutout);
            }
            if let Some(clip) = clip {
                node = node.clip(*clip);
            }
            node
        }
        Primitive::Shadow {
            rect,
            radii,
            sigma,
            color,
            gradient,
            cutout,
            clip,
            opacity,
        } => {
            let mut node = Node::shadow(RoundedRect::new(*rect, *radii), *sigma)
                .fill(*color)
                .opacity(*opacity)
                .z(z);
            if let Some(gradient) = gradient {
                node = node.fill_gradient(**gradient);
            }
            if let Some(cutout) = cutout {
                node = node.cut_out(*cutout);
            }
            if let Some(clip) = clip {
                node = node.clip(*clip);
            }
            node
        }
        Primitive::Arc {
            center,
            radius,
            start_angle,
            sweep_angle,
            width,
            color,
            clip,
        } => {
            let mut node = Node::arc(*center, *radius, *start_angle, *sweep_angle)
                .stroke(Stroke::new(*width, *color))
                .z(z);
            if let Some(clip) = clip {
                node = node.clip(*clip);
            }
            node
        }
        Primitive::Text {
            rect,
            paint,
            color,
            gradient,
            clip,
            ..
        } => {
            let mut node = Node::text(*rect, paint.clone()).fill(*color).z(z);
            if let Some(gradient) = gradient {
                node = node.fill_gradient(**gradient);
            }
            if let Some(clip) = clip {
                node = node.clip(*clip);
            }
            node
        }
        Primitive::Icon {
            rect,
            spec,
            scale,
            color,
            gradient,
            clip,
        } => {
            let mut node = Node::icon(*rect, spec.scaled(*scale)).fill(*color).z(z);
            if let Some(gradient) = gradient {
                node = node.fill_gradient(**gradient);
            }
            if let Some(clip) = clip {
                node = node.clip(*clip);
            }
            node
        }
        Primitive::Image {
            rect,
            tint,
            gradient,
            pixelated,
            radii,
            turn,
            mirror,
            clip,
            ..
        } => {
            let mut node = match resolved {
                Some(texture) => {
                    let image = Node::image(*rect, texture)
                        .source(image_source(*pixelated, *mirror))
                        .radii(*radii)
                        .rotation(*turn)
                        .fill(*tint);
                    match gradient {
                        Some(gradient) => image.fill_gradient(**gradient),
                        None => image,
                    }
                }
                None => Node::rounded_rect(*rect, CornerRadii::ZERO).fill(Color::TRANSPARENT),
            }
            .z(z);
            if let Some(clip) = clip {
                node = node.clip(*clip);
            }
            node
        }
        Primitive::Segment {
            rect,
            rotation,
            color,
            clip,
        } => {
            let mut node = Node::capsule(*rect, *rotation).fill(*color).z(z);
            if let Some(clip) = clip {
                node = node.clip(*clip);
            }
            node
        }
    }
}

fn image_source(pixelated: bool, mirror: Transform) -> ImageSource {
    ImageSource::whole().pixelated(pixelated).transform(mirror)
}

fn update_in_place(
    scene: &mut Scene,
    slot: &mut PrimSlot,
    new: &Primitive,
    resolved: Option<TextureId>,
) -> bool {
    match (&slot.prim, new) {
        (
            Primitive::Quad { blur: old_blur, .. },
            Primitive::Quad {
                rect,
                fill,
                fill_gradient,
                radii,
                stroke,
                stroke_gradient,
                blur,
                lens,
                cutout,
                clip,
                opacity,
            },
        ) => {
            let mut node = scene.edit(slot.node);
            node.rect(*rect)
                .fill(*fill)
                .fill_gradient(fill_gradient.as_deref().copied())
                .radii(*radii)
                .stroke(stroke.map(|(width, color)| Stroke::new(width, color)))
                .stroke_gradient(stroke_gradient.as_deref().copied())
                .lens(*lens)
                .cut_out(*cutout)
                .clip(*clip)
                .opacity(*opacity);
            if old_blur != blur {
                match blur {
                    Some(sigma) => node.backdrop(Blur::new(*sigma)),
                    None => node.clear_backdrop(),
                };
            }
            true
        }
        (
            Primitive::Shadow { .. },
            Primitive::Shadow {
                rect,
                radii,
                sigma,
                color,
                gradient,
                cutout,
                clip,
                opacity,
            },
        ) => {
            scene
                .edit(slot.node)
                .shadow(RoundedRect::new(*rect, *radii), *sigma)
                .fill(*color)
                .fill_gradient(gradient.as_deref().copied())
                .cut_out(*cutout)
                .clip(*clip)
                .opacity(*opacity);
            true
        }
        (
            Primitive::Arc { .. },
            Primitive::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
                width,
                color,
                clip,
            },
        ) => {
            scene
                .edit(slot.node)
                .rect(Rect::from_circle(*center, *radius))
                .arc(*start_angle, *sweep_angle)
                .stroke(Stroke::new(*width, *color))
                .clip(*clip);
            true
        }
        (
            Primitive::Text { .. },
            Primitive::Text {
                rect,
                paint,
                color,
                gradient,
                clip,
                ..
            },
        ) => {
            scene
                .edit(slot.node)
                .rect(*rect)
                .fill(*color)
                .fill_gradient(gradient.as_deref().copied())
                .clip(*clip)
                .text(paint.clone());
            true
        }
        (
            Primitive::Icon {
                spec: old_spec,
                scale: old_scale,
                ..
            },
            Primitive::Icon {
                rect,
                spec,
                scale,
                color,
                gradient,
                clip,
            },
        ) => {
            let mut node = scene.edit(slot.node);
            node.rect(*rect)
                .fill(*color)
                .fill_gradient(gradient.as_deref().copied())
                .clip(*clip);
            if old_spec != spec || old_scale != scale {
                node.icon(spec.scaled(*scale));
            }
            true
        }
        (
            Primitive::Image { .. },
            Primitive::Image {
                rect,
                tint,
                gradient,
                pixelated,
                radii,
                turn,
                mirror,
                clip,
                ..
            },
        ) => {
            if slot.resolved.texture().is_some() != resolved.is_some() {
                return false;
            }
            let mut node = scene.edit(slot.node);
            node.rect(*rect).clip(*clip);
            if let Some(texture) = resolved {
                node.image(texture)
                    .source(image_source(*pixelated, *mirror))
                    .radii(*radii)
                    .rotation(*turn)
                    .fill(*tint)
                    .fill_gradient(gradient.as_deref().copied());
            }
            true
        }
        (
            Primitive::Segment { .. },
            Primitive::Segment {
                rect,
                rotation,
                color,
                clip,
            },
        ) => {
            scene
                .edit(slot.node)
                .rect(*rect)
                .rotation(*rotation)
                .fill(*color)
                .clip(*clip);
            true
        }
        _ => false,
    }
}

pub(crate) fn refresh_images(sink: &mut Sink<'_>, slots: &mut [PrimSlot]) {
    for slot in slots.iter_mut() {
        let Some(image) = slot.image else {
            continue;
        };
        let resolved = resolve(&slot.prim, sink.resources);
        sink.images.resolved(image, resolved);
        if let (Resolved::Source(texture), Resolved::Source(_)) = (resolved, slot.resolved) {
            let Changes { repaint, serial } = changes(&slot.prim, slot.version, sink.resources);
            slot.version = serial;
            if let Some(areas) = repainted_areas(&slot.prim, &repaint) {
                sink.scene
                    .edit(slot.node)
                    .repaint_image_within(texture, areas);
                slot.resolved = resolved;
                continue;
            }
        } else {
            slot.version = latest(&slot.prim, sink.resources);
        }
        if resolved == slot.resolved {
            continue;
        }
        let prim = slot.prim.clone();
        if !update_in_place(sink.scene, slot, &prim, resolved.texture()) {
            sink.scene.remove(slot.node);
            slot.node = insert_prim(sink.scene, &prim, resolved.texture(), slot.placement);
        }
        slot.resolved = resolved;
    }
}
