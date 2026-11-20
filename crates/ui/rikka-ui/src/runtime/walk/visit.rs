use std::hash::{Hash, Hasher};

use rikka_renderer::{GroupClip, LayerId};
use rikka_types::{Point, Rect, RoundedRect};

use crate::paint::{READS_CHILDREN, READS_CLIP, READS_VIEWPORT, on_grid, settled};
use crate::runtime::instance::Instance;
use crate::runtime::layout::{Layouts, Node};
use crate::runtime::scene_sync::{Frame, GroupScroll, Placing, Sink, apply_prims, show_prims};
use crate::runtime::table::{
    CULLED, Clipping, ENTERED, PAINT, PENDING, Reach, TRACKS, TRACKS_BELOW, Table, ZOOMED,
};
use crate::runtime::visuals::{self, Look};
use crate::runtime::walk::{BLEED, Clip, Context, EnterContext, NodeGeometry, Space, Walk};
use crate::view::Behavior;

impl Walk<'_> {
    pub(super) fn visit(
        &mut self,
        node: Node,
        context: Context,
        force: bool,
    ) -> Option<EnterContext> {
        let flags = self.table.flags(node);
        if flags & PENDING == 0 && !force && !self.all && self.stands(node, flags, &context) {
            return None;
        }
        self.table.lower(node, PENDING);
        let mut instance = self.table.take(node);
        let entered = self.enter(node, &mut instance, context, force, flags & PAINT != 0);
        self.table.put(node, instance);
        entered
    }

    fn stands(&self, node: Node, flags: u16, context: &Context) -> bool {
        if flags & CULLED != 0 {
            return culling(context).is_some_and(|seen| !seen.touches(&self.table.placed(node)));
        }
        let Some(last) = self.table.get(node).last_context.as_ref() else {
            return false;
        };
        *last == *context
            || (flags & (TRACKS | TRACKS_BELOW) == 0
                && context.carries(last)
                && self.table.reach(node).holds(culling(context)))
    }

    fn enter(
        &mut self,
        node: Node,
        instance: &mut Instance,
        context: Context,
        force: bool,
        requested: bool,
    ) -> Option<EnterContext> {
        let before = (self.table.placed(node), self.table.clipping(node));
        instance.last_context = Some(context);
        let target = self.layouts.rect(node);
        let content = self.layouts.content(node);
        instance.lane = self.layouts.lane(node);
        let (placed, framing) = visuals::frame(instance, target, self.transaction, self.clock);
        instance.placed = Some(placed);
        let shift = instance
            .leaving
            .as_mut()
            .map_or(Point::ZERO, |leaving| leaving.shift(context.origin));
        let local = placed.translate(shift);
        let bounds = local.translate(context.origin);
        instance.content_size = content;
        let culled = self.table.has(node, CULLED);
        if let Some(clip) = context.clip
            && !instance.paints_offscreen
            && !clip.shape.rect.inflate(BLEED).touches(&bounds)
        {
            self.table.set_placed(node, local);
            self.table.set_reach(node, Reach::culled(local));
            self.table.lower(node, TRACKS | TRACKS_BELOW);
            if framing {
                self.out.active.push(instance.id);
            }
            if !culled {
                self.out.moved = true;
                show_prims(&mut self.sink, &mut instance.prims, false);
                cull(instance);
                self.table.raise(node, CULLED);
                self.table.set_extent(node, Rect::ZERO);
                hide_subtree(self.table, self.layouts, &mut self.sink, node);
            }
            if requested || self.all {
                self.table.raise(node, PAINT);
            }
            if self.all {
                defer_subtree(self.table, self.layouts, node);
            }
            return None;
        }
        self.table.lower(node, CULLED);
        let force = force || culled;
        crate::stats::visited();

        let Look {
            opacity,
            scale: zoom,
            about_anchor,
            nudge,
            background,
            reveal,
            moving,
        } = match visuals::look(instance, bounds, self.clock, self.debut) {
            Some(look) => look,
            None => Look::PLAIN,
        };
        if instance.leaving.is_some_and(|leaving| leaving.done) {
            self.out.departed.push(node);
        }
        if moving || framing {
            self.out.active.push(instance.id);
        }
        let bounds = bounds.translate(nudge);
        self.table.set_placed(node, local.translate(nudge));
        self.table.set_reach(
            node,
            match instance.paints_offscreen {
                true => Reach::OPEN,
                false => Reach::shown(local),
            },
        );
        let revealed = reveal.map(|reveal| Clip {
            shape: RoundedRect::rect(reveal.translate(nudge)),
            space: Space::Volatile,
        });
        let inherited = Clip::narrowed(context.clip, revealed);
        let loose = Clip::narrowed(context.loose, revealed);
        let layer = self.settle_layer(&mut instance.visual_fade, opacity, context.layer);
        let (mut scale, mut offset) = (context.scale, context.offset);
        if zoom != 1.0 {
            let anchored = instance.anchor.is_some() && about_anchor;
            let pivot = match anchored.then(|| self.pivots.get(&node)).flatten() {
                Some(pivot) => bounds.origin + *pivot,
                None => bounds.center(),
            };
            scale = context.scale * zoom;
            offset = context.offset + pivot * context.scale * (1.0 - zoom);
        }
        let behavior = instance.behavior;
        let raises = instance.anchor.is_some() || behavior.has(Behavior::OVERLAY);
        let (inherited, loose, bounded, stack) = match raises {
            true => (None, None, false, self.table.labels.label(node)),
            false => (inherited, loose, context.bounded, context.stack),
        };
        let own_clip = revealed.map(|clip| clip.shape.rect.translate(Point::ZERO - context.origin));
        let own = instance.clip_rect(bounds).map(|shape| Clip {
            shape,
            space: Space::Group(context.frame),
        });
        let base = Clip::narrowed(inherited, own);
        let focused = self.focus == Some(instance.id);
        let focus_within = context.focus_within || focused;
        let drawn = NodeGeometry {
            bounds,
            room: instance.room(bounds),
            content,
            viewport: self.viewport,
            scale,
            offset,
            base,
            loose: Clip::narrowed(loose, own),
            bounded,
            hovered: self.table.has(node, ENTERED),
            focused,
            focus_within,
            background,
            children: children_digest(self.layouts, node, instance.outputs.reads),
            frame: context.frame,
            layer,
            stack,
        };
        let repaint = requested
            || self.all
            || instance
                .drawn
                .as_ref()
                .is_none_or(|was| stale(was, &drawn, instance, behavior));
        if repaint {
            self.paint(node, instance, &drawn);
        } else if let Some(was) = instance.drawn.filter(|was| moved(was, &drawn)) {
            self.replace(node, instance, &was, &drawn);
        }
        if culled {
            show_prims(&mut self.sink, &mut instance.prims, true);
        }
        if let Some(was) = instance.drawn
            && !instance.moves
            && !self.layouts.children(node).is_empty()
            && travelled(&was, &drawn)
        {
            instance.moves = true;
            self.out.shifted.push(node);
        }
        instance.drawn = Some(drawn);
        instance.refresh_behavior();
        self.settle_tracking(node, instance);
        let owned = self.table.islands.owned(node);
        if owned.is_none() && self.layouts.children(node).is_empty() {
            let opacity = instance.outputs.subtree_opacity;
            self.settle_layer(&mut instance.subtree_fade, opacity, layer);
            self.table.set_clipping(
                node,
                Clipping {
                    own: own_clip,
                    children: None,
                    raises,
                },
            );
            self.note_moved(node, before, culled);
            self.seal(node, instance, context.origin, bounds.origin);
            return None;
        }

        let children_own = instance
            .outputs
            .children_clip
            .or_else(|| instance.clip_rect(bounds));
        let group = owned.map(|island| {
            *self
                .table
                .islands
                .group_mut(island)
                .get_or_insert_with(|| self.sink.scene.create_group())
        });
        let offsets = instance.offsets_children();
        let subtree_opacity = instance.outputs.subtree_opacity;
        let child_layer = self.settle_layer(&mut instance.subtree_fade, subtree_opacity, layer);
        if let Some((factor, pivot)) = instance.outputs.subtree_scale {
            offset += pivot * scale * (1.0 - factor);
            scale *= factor;
        }
        let shift = instance
            .widget
            .children_offset(instance.state.as_ref(), bounds, content);
        if shift != Point::ZERO && !offsets {
            instance.shifts = true;
            self.out.shifted.push(node);
        }
        if self.table.inner(node) != shift {
            self.table.set_inner(node, shift);
            self.out.moved = true;
        }
        let child_origin = bounds.origin + shift;
        let frame = match group {
            Some(group) => Frame::at(group, child_origin * scale + offset),
            None => context.frame,
        };
        let framed_by = match group {
            Some(_) if !offsets => frame,
            _ => context.frame,
        };
        let children_clip = children_own.map(|shape| Clip {
            shape,
            space: Space::Group(framed_by),
        });
        self.table.set_clipping(
            node,
            Clipping {
                own: own_clip,
                children: children_own
                    .map(|shape| shape.rect.translate(Point::ZERO - context.origin)),
                raises,
            },
        );
        let child_clip = Clip::narrowed(inherited, children_clip);
        let child_loose = Clip::narrowed(loose, children_clip);
        let (child_loose, child_bounded) = match group {
            Some(group) => {
                let placed = |clip: Clip| {
                    let shape = physical(clip.shape, scale, offset);
                    RoundedRect::new(
                        settled(shape.rect.translate(Point::ZERO - context.frame.origin)),
                        shape.radii,
                    )
                };
                let clip = match bounded {
                    true => child_loose
                        .map_or(GroupClip::Inherit, |clip| GroupClip::Narrow(placed(clip))),
                    false => GroupClip::Only(child_loose.map(placed)),
                };
                self.sink.scene.set_group_clip(group, clip);
                (None, true)
            }
            None => (child_loose, bounded),
        };
        self.note_moved(node, before, culled);
        Some(EnterContext {
            within: Context {
                origin: child_origin,
                clip: child_clip,
                loose: child_loose,
                bounded: child_bounded,
                layer: child_layer,
                scale,
                offset,
                frame,
                focus_within,
                stack,
            },
            force,
        })
    }

    fn settle_tracking(&mut self, node: Node, instance: &Instance) {
        let tracks = !plain(instance, instance.behavior)
            || instance.hosts
            || instance.outputs.topmost
            || self.table.has(node, ZOOMED);
        match tracks {
            true => self.table.raise(node, TRACKS),
            false => self.table.lower(node, TRACKS),
        }
    }

    pub(super) fn close(&mut self, node: Node, within: Context) {
        let mut instance = self.table.take(node);
        let origin = instance
            .last_context
            .map_or(Point::ZERO, |context| context.origin);
        let extent = self.seal(node, &mut instance, origin, within.origin);
        if let (Some(island), Some(last_context)) =
            (self.table.islands.owned(node), instance.last_context)
            && let Some(group) = self.table.islands.island(island).group
        {
            let offset = within.frame.origin - last_context.frame.origin;
            let shown = extent
                .translate(origin)
                .inflate(BLEED)
                .scale(within.scale)
                .translate(within.offset - last_context.frame.origin);
            self.sink
                .scene
                .set_group(group, last_context.frame.group, offset, shown);
        }
        self.table.put(node, instance);
    }

    fn note_moved(&mut self, node: Node, before: (Rect, Clipping), culled: bool) {
        if culled || (self.table.placed(node), self.table.clipping(node)) != before {
            self.out.moved = true;
        }
    }

    fn seal(&mut self, node: Node, instance: &mut Instance, origin: Point, within: Point) -> Rect {
        let local = self.table.placed(node);
        let offset = within - origin;
        let raises = self.table.clipping(node).raises;
        let mut extent = instance.widget.hit_area(local);
        let mut reach = self.table.reach(node);
        let mut below = false;
        for &child in self.layouts.children(node) {
            extent = extent.union(&self.table.extent(child).translate(offset));
            below |= self.table.flags(child) & (TRACKS | TRACKS_BELOW) != 0;
            if !raises {
                reach.join(self.table.reach(child), offset);
            }
        }
        if extent != self.table.extent(node) {
            self.out.moved = true;
        }
        self.table.set_extent(node, extent);
        self.table.set_reach(node, reach);
        match below {
            true => self.table.raise(node, TRACKS_BELOW),
            false => self.table.lower(node, TRACKS_BELOW),
        }
        if let Some(revealing) = self.reveal.take_if(|revealing| revealing.reaches(node)) {
            let bounds = local.translate(origin);
            let room = instance.room(bounds);
            match instance.widget.reveal(
                instance.state.as_mut(),
                room,
                instance.content_size,
                revealing.rect,
                revealing.reveal,
            ) {
                true => {
                    self.out.active.push(instance.id);
                    instance.refresh_behavior();
                }
                false => {
                    self.reveal = self.layouts.parent(node).map(|parent| {
                        revealing.passed_to(parent, self.table.bounds(self.layouts, parent))
                    });
                }
            }
        }
        extent
    }

    fn replace(
        &mut self,
        node: Node,
        instance: &mut Instance,
        was: &NodeGeometry,
        now: &NodeGeometry,
    ) {
        let then = was.bounds.origin * was.scale + was.offset;
        let here = now.bounds.origin * now.scale + now.offset;
        let shift = here - then;
        let place =
            |clip: Option<Clip>| clip.map(|clip| physical(clip.shape, now.scale, now.offset));
        let (base, loose) = (place(now.base), place(now.loose));
        self.spare.scratch.clear();
        self.spare.scratch.extend(
            instance
                .prims
                .iter()
                .map(|slot| slot.repainted(shift, base, loose)),
        );
        for pane in &mut instance.outputs.glass {
            pane.rect = pane.rect.translate(shift);
        }
        instance.outputs.opaque.retain_mut(|shape| {
            shape.rect = shape.rect.translate(shift);
            let Some(clip) = base else {
                return true;
            };
            match shape.intersect(clip) {
                Some(cached) => {
                    *shape = cached;
                    true
                }
                None => false,
            }
        });
        for window in &mut instance.outputs.windows {
            *window = window.clone().translated(shift);
        }
        for section in &mut instance.outputs.sections {
            section.shown = section.shown.translate(shift);
        }
        let moved = now.bounds.origin - was.bounds.origin;
        let outputs = &mut instance.outputs;
        if let Some(clip) = &mut outputs.children_clip {
            clip.rect = clip.rect.translate(moved);
        }
        if let Some((_, pivot)) = &mut outputs.subtree_scale {
            *pivot += moved;
        }
        instance.relocate(now.bounds.origin);
        if was.layer != now.layer {
            self.resettle_fades(instance, now.layer);
        }
        self.apply(node, instance, now);
    }

    pub(super) fn apply(&mut self, node: Node, instance: &mut Instance, drawn: &NodeGeometry) {
        let base = match drawn.loose.map(|clip| clip.space) {
            Some(Space::Group(frame)) => frame,
            _ => Frame::default(),
        };
        self.settle_sections(instance, drawn.frame);
        let placing = Placing {
            frame: drawn.frame,
            layer: drawn.layer,
            fades: &instance.fades,
            sections: &instance.sections,
            stack: drawn.stack,
            labels: &self.table.labels,
            node,
            base,
            bounded: drawn.bounded,
        };
        apply_prims(
            &mut self.sink,
            &mut instance.prims,
            &mut self.spare.scratch,
            placing,
            &mut self.spare.regrouping,
        );
        let imaged = instance.prims.iter().any(|slot| slot.image.is_some());
        if imaged != instance.imaged {
            instance.imaged = imaged;
            self.out.imaged.push((node, imaged));
        }
    }

    fn settle_sections(&mut self, instance: &mut Instance, frame: Frame) {
        let section_count = instance.outputs.sections.len();
        if instance.sections.len() > section_count {
            for scrolled in instance.sections.drain(section_count..) {
                self.sink.scene.remove_group(scrolled.group);
            }
        }
        while instance.sections.len() < section_count {
            let group = self.sink.scene.create_group();
            instance.sections.push(GroupScroll {
                group,
                by: Point::ZERO,
            });
        }
        for (section, scrolled) in instance.outputs.sections.iter().zip(&mut instance.sections) {
            scrolled.by = section.by;
            self.sink.scene.set_group(
                scrolled.group,
                frame.group,
                section.by,
                section.shown.translate(Point::ZERO - frame.origin),
            );
        }
    }

    fn settle_layer(
        &mut self,
        layer_slot: &mut Option<LayerId>,
        opacity: f32,
        outer: Option<LayerId>,
    ) -> Option<LayerId> {
        if opacity >= 1.0 {
            if let Some(layer) = layer_slot.take() {
                self.sink.scene.remove_layer(layer);
            }
            return outer;
        }
        let layer = *layer_slot.get_or_insert_with(|| self.sink.scene.create_layer());
        self.sink.scene.set_layer(layer, outer, opacity);
        Some(layer)
    }

    pub(super) fn settle_fades(&mut self, instance: &mut Instance, outer: Option<LayerId>) {
        let section_count = self.spare.fades.len();
        if instance.fades.len() > section_count {
            for layer in instance.fades.drain(section_count..) {
                self.sink.scene.remove_layer(layer);
            }
        }
        while instance.fades.len() < section_count {
            instance.fades.push(self.sink.scene.create_layer());
        }
        for (fade, layer) in self.spare.fades.iter().zip(&instance.fades) {
            let within = fade.within.map(|above| instance.fades[above]).or(outer);
            self.sink.scene.set_layer(*layer, within, fade.opacity);
        }
        instance.outputs.fades.clone_from(&self.spare.fades);
    }

    fn resettle_fades(&mut self, instance: &mut Instance, outer: Option<LayerId>) {
        std::mem::swap(&mut self.spare.fades, &mut instance.outputs.fades);
        self.settle_fades(instance, outer);
    }
}

fn culling(context: &Context) -> Option<Rect> {
    context.clip.map(|clip| {
        clip.shape
            .rect
            .inflate(BLEED)
            .translate(Point::ZERO - context.origin)
    })
}

pub(crate) fn physical(shape: RoundedRect, scale: f32, offset: Point) -> RoundedRect {
    RoundedRect::new(
        shape.rect.scale(scale).translate(offset),
        shape.radii * scale,
    )
}

fn cull(instance: &mut Instance) {
    instance.asked_build = false;
    instance.wants_build = false;
}

fn hide_subtree(table: &mut Table, layouts: &Layouts, sink: &mut Sink<'_>, node: Node) {
    for &child in layouts.children(node) {
        if !table.holds(child) {
            continue;
        }
        let instance = table.get_mut(child);
        show_prims(sink, &mut instance.prims, false);
        cull(instance);
        table.raise(child, CULLED);
        table.set_extent(child, Rect::ZERO);
        hide_subtree(table, layouts, sink, child);
    }
}

fn defer_subtree(table: &mut Table, layouts: &Layouts, node: Node) {
    for &child in layouts.children(node) {
        if table.holds(child) {
            table.raise(child, PAINT);
            defer_subtree(table, layouts, child);
        }
    }
}

fn plain(instance: &Instance, behavior: Behavior) -> bool {
    !instance.outputs.volatile
        && instance.outputs.reads & (READS_CLIP | READS_VIEWPORT) == 0
        && !behavior.has(Behavior::TRACKS_PLACE)
}

fn stale(was: &NodeGeometry, now: &NodeGeometry, instance: &Instance, behavior: Behavior) -> bool {
    let reads = instance.outputs.reads;
    let plain = plain(instance, behavior);
    let room = |drawn: &NodeGeometry| {
        let room = drawn.room.translate(Point::ZERO - drawn.bounds.origin);
        (
            on_grid(room.x()),
            on_grid(room.y()),
            room.width(),
            room.height(),
        )
    };
    was.bounds.size != now.bounds.size
        || was.scale != now.scale
        || was.content != now.content
        || room(was) != room(now)
        || was.hovered != now.hovered
        || was.focused != now.focused
        || was.focus_within != now.focus_within
        || was.background != now.background
        || (reads & READS_VIEWPORT != 0 && was.viewport != now.viewport)
        || (reads & READS_CHILDREN != 0 && was.children != now.children)
        || (!plain && moved(was, now))
}

fn moved(was: &NodeGeometry, now: &NodeGeometry) -> bool {
    was.bounds.origin != now.bounds.origin
        || was.offset != now.offset
        || was.base != now.base
        || was.loose != now.loose
        || was.bounded != now.bounded
        || was.frame != now.frame
        || was.layer != now.layer
        || was.stack != now.stack
}

fn children_digest(layouts: &Layouts, node: Node, reads: u8) -> u64 {
    if reads & READS_CHILDREN == 0 {
        return 0;
    }
    let mut hasher = rustc_hash::FxHasher::default();
    for &child in layouts.children(node) {
        let rect = layouts.rect(child);
        [rect.x(), rect.y(), rect.width(), rect.height()]
            .map(f32::to_bits)
            .hash(&mut hasher);
    }
    hasher.finish()
}

fn travelled(was: &NodeGeometry, now: &NodeGeometry) -> bool {
    let within = |drawn: &NodeGeometry| {
        let at = drawn.bounds.origin * drawn.scale + drawn.offset - drawn.frame.exact;
        (on_grid(at.x), on_grid(at.y))
    };
    was.frame.group == now.frame.group
        && was.scale == now.scale
        && was.bounds.size == now.bounds.size
        && within(was) != within(now)
}
