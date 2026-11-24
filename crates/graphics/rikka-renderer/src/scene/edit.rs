use std::sync::Arc;

use rikka_text::{IconSpec, TextPaint};
use rikka_types::{Color, CornerRadii, Gradient, Lens, Rect, RoundedRect};

use crate::fx::{BackdropCache, Joined, PostEffect};
use crate::scene::audience::Audience;
use crate::scene::dirt::{BOUNDS, CONTENT, Dirt, KEPT, LAYER, MOVED, ORDER, SHOWN};
use crate::scene::image_source::ImageSource;
use crate::scene::node::{Gradients, GroupId, LayerId, Node, NodeId, ShadowExtent, Shape, Stroke};
use crate::scene::scene_impl::Scene;
use rikka_gpu::texture::TextureId;

#[must_use = "a node edit applies its changes when it is dropped"]
pub struct NodeEdit<'a> {
    scene: &'a mut Scene,
    id: NodeId,
    before: Option<Rect>,
    was_visible: bool,
    audience: Audience,
    dirt: Dirt,
    repainted: bool,
}

impl Scene {
    pub fn edit(&mut self, id: NodeId) -> NodeEdit<'_> {
        let (before, was_visible, audience) = self
            .get(id)
            .map_or((None, false, Audience::Everyone), |node| {
                (Some(self.bounds_of(node)), node.visible, node.audience)
            });
        NodeEdit {
            scene: self,
            id,
            before,
            was_visible,
            audience,
            dirt: 0,
            repainted: false,
        }
    }
}

impl NodeEdit<'_> {
    pub fn changed(&self) -> bool {
        self.repainted || self.dirt & !KEPT != 0
    }

    pub fn rect(&mut self, rect: Rect) -> &mut Self {
        let dirt = match self
            .scene
            .get(self.id)
            .is_some_and(|node| node.rect.size == rect.size)
        {
            true => MOVED,
            false => CONTENT,
        };
        self.touch(dirt, |node| replace(&mut node.rect, rect))
    }

    pub fn fill(&mut self, fill: Color) -> &mut Self {
        self.assign(fill, |node| &mut node.fill)
    }

    pub fn fill_gradient(&mut self, gradient: impl Into<Option<Gradient>>) -> &mut Self {
        let gradient = gradient.into();
        self.gradients(|gradients| gradients.fill = gradient)
    }

    pub fn stroke_gradient(&mut self, gradient: impl Into<Option<Gradient>>) -> &mut Self {
        let gradient = gradient.into();
        self.gradients(|gradients| gradients.stroke = gradient)
    }

    fn gradients(&mut self, change: impl FnOnce(&mut Gradients)) -> &mut Self {
        self.apply(|node| {
            let next = Gradients::changed(node.gradients.as_deref(), change);
            replace_boxed(&mut node.gradients, next)
        })
    }

    pub fn shadow(&mut self, shape: RoundedRect, sigma: f32) -> &mut Self {
        let extent = ShadowExtent::of(shape, sigma);
        self.rect(extent.rect);
        self.apply(|node| match &mut node.shape {
            Shape::Shadow { radii, sigma } => {
                replace(radii, extent.radii) | replace(sigma, extent.sigma)
            }
            _ => false,
        })
    }

    pub fn stroke(&mut self, stroke: impl Into<Option<Stroke>>) -> &mut Self {
        let stroke = stroke.into();
        let Some(node) = self.node_mut() else {
            return self;
        };
        if node.stroke == stroke {
            return self;
        }
        let was = std::mem::replace(&mut node.stroke, stroke);
        let width = [was, stroke]
            .into_iter()
            .flatten()
            .map(|stroke| stroke.width)
            .fold(0.0, f32::max);
        let rim = match &node.shape {
            Shape::Rect => Some(RoundedRect::rect(node.rect)),
            Shape::RoundedRect { radii } => Some(RoundedRect::new(node.rect, *radii)),
            _ => None,
        };
        let group = node.group;
        self.repaint_within(rim.map(|shape| shape.rim(width)), group);
        self
    }

    pub fn clip(&mut self, clip: impl Into<Option<RoundedRect>>) -> &mut Self {
        let clip = clip.into();
        let Some(node) = self.node_mut() else {
            return self;
        };
        if node.clip == clip {
            return self;
        }
        let was = std::mem::replace(&mut node.clip, clip);
        let space = node.clip_group;
        let corners = was
            .zip(clip)
            .and_then(|(was, now)| now.changed_corners(&was));
        self.repaint_within(corners, space);
        self
    }

    pub fn cut_out(&mut self, cutout: impl Into<Option<RoundedRect>>) -> &mut Self {
        let cutout = cutout.into();
        self.apply(|node| replace_boxed(&mut node.cutout, cutout))
    }

    pub fn clip_in(&mut self, group: impl Into<Option<GroupId>>) -> &mut Self {
        let group = group.into();
        let mut moved = None;
        self.apply(|node| {
            let from = node.clip_group;
            let changed = replace(&mut node.clip_group, group);
            if changed {
                moved = Some((node.group, from));
            }
            changed
        });
        if let Some((own, from)) = moved {
            self.scene.move_clip(self.id, own, from, group);
        }
        self
    }

    pub fn clipped_by_group(&mut self, clipped: bool) -> &mut Self {
        let mut toggled = None;
        self.apply(|node| {
            let was = node.escapes_group_clip();
            let changed = replace(&mut node.clipped_by_group, clipped);
            if changed {
                let now = node.escapes_group_clip();
                toggled = Some((was, now));
            }
            changed
        });
        if let Some((was, now)) = toggled {
            self.scene.count_unclipped(self.id, was, now);
        }
        self
    }

    pub fn opacity(&mut self, opacity: f32) -> &mut Self {
        self.assign(opacity.clamp(0.0, 1.0), |node| &mut node.opacity)
    }

    pub fn visible(&mut self, visible: bool) -> &mut Self {
        self.touch(SHOWN, |node| replace(&mut node.visible, visible))
    }

    pub fn audience(&mut self, audience: Audience) -> &mut Self {
        self.touch(SHOWN, |node| replace(&mut node.audience, audience))
    }

    pub fn lens(&mut self, lens: impl Into<Option<Lens>>) -> &mut Self {
        let lens = lens.into();
        self.apply(|node| replace_boxed(&mut node.lens, lens))
    }

    pub fn joined(&mut self, joined: impl Into<Option<Joined>>) -> &mut Self {
        let joined = joined.into();
        self.apply(|node| replace_boxed(&mut node.joined, joined))
    }

    pub fn opaque_within(&mut self, opaque: impl Into<Option<Rect>>) -> &mut Self {
        let opaque = opaque.into();
        self.touch(KEPT, |node| replace(&mut node.opaque, opaque))
    }

    pub fn z(&mut self, z: i32) -> &mut Self {
        self.touch(ORDER, |node| replace(&mut node.z, z))
    }

    pub fn stack(&mut self, stack: u64) -> &mut Self {
        self.touch(ORDER, |node| replace(&mut node.stack, stack))
    }

    pub fn order(&mut self, label: u64) -> &mut Self {
        self.touch(ORDER, |node| replace(&mut node.label, label))
    }

    pub fn layer(&mut self, layer: impl Into<Option<LayerId>>) -> &mut Self {
        let layer = layer.into();
        self.touch(ORDER | LAYER, |node| replace(&mut node.layer, layer))
    }

    pub fn radii(&mut self, radii: impl Into<CornerRadii>) -> &mut Self {
        let radii = radii.into();
        let Some(node) = self.node_mut() else {
            return self;
        };
        let rect = node.rect;
        let group = node.group;
        let (current, turned) = match &mut node.shape {
            Shape::RoundedRect { radii: current } => (current, false),
            Shape::Image {
                radii: current,
                rotation,
                ..
            } => (current, *rotation != 0.0),
            _ => return self,
        };
        if *current == radii {
            return self;
        }
        let was = RoundedRect::new(rect, std::mem::replace(current, radii));
        let corners = RoundedRect::new(rect, radii)
            .changed_corners(&was)
            .filter(|_| !turned);
        self.repaint_within(corners, group);
        self
    }

    pub fn rotation(&mut self, angle: f32) -> &mut Self {
        self.apply(|node| match &mut node.shape {
            Shape::Image { rotation, .. } | Shape::Capsule { rotation } => replace(rotation, angle),
            _ => false,
        })
    }

    pub fn arc(&mut self, start_angle: f32, sweep_angle: f32) -> &mut Self {
        self.apply(|node| match &mut node.shape {
            Shape::Arc {
                start_angle: start,
                sweep_angle: sweep,
            } => replace(start, start_angle) | replace(sweep, sweep_angle),
            _ => false,
        })
    }

    pub fn text(&mut self, paint: impl Into<TextPaint>) -> &mut Self {
        let paint = paint.into();
        let unchanged = self
            .scene
            .get(self.id)
            .is_some_and(|node| matches!(&node.shape, Shape::Text(current) if *current == paint));
        if unchanged {
            return self;
        }
        let Some(rows) = self.recolored_rows(&paint) else {
            return self.apply(|node| match &mut node.shape {
                Shape::Text(current) => {
                    *current = paint;
                    true
                }
                _ => false,
            });
        };
        let id = self.id;
        if let Some(node) = self.scene.nodes.get_mut(id.index, id.generation)
            && let Shape::Text(current) = &mut node.shape
        {
            *current = paint;
        }
        self.repainted = true;
        self.scene.mark(id, CONTENT);
        if let Some(rows) = rows
            && self.is_visible()
        {
            self.scene.damage.add_for(self.audience, rows);
        }
        self
    }

    fn recolored_rows(&self, paint: &TextPaint) -> Option<Option<Rect>> {
        self.before?;
        let node = self.scene.get(self.id)?;
        let Shape::Text(current) = &node.shape else {
            return None;
        };
        if !Arc::ptr_eq(&current.layout, &paint.layout)
            || current.scale != paint.scale
            || current.color_scheme != paint.color_scheme
        {
            return None;
        }
        let changed = current.inks.differing(&paint.inks)?;
        let rows = paint
            .layout
            .rows()
            .skip_while(|row| {
                let range = row.text_range();
                range.end.max(range.start + 1) <= changed.start
            })
            .take_while(|row| row.text_range().start < changed.end)
            .fold(None, |span: Option<(f32, f32)>, row| {
                Some(span.map_or((row.top(), row.bottom()), |(top, bottom)| {
                    (top.min(row.top()), bottom.max(row.bottom()))
                }))
            });
        let Some((top, bottom)) = rows else {
            return Some(None);
        };
        let scale = paint.scale;
        let reach = paint.layout.font_size() * scale;
        let origin = node.rect.min_y().round() + self.scene.offset_of(node.group).y;
        let bounds = self.scene.bounds_of(node);
        let from = origin + top * scale - reach;
        let to = origin + bottom * scale + reach;
        Some(Rect::new(bounds.x(), from, bounds.width(), to - from).intersection(&bounds))
    }

    pub fn icon(&mut self, spec: IconSpec) -> &mut Self {
        self.apply(|node| match &mut node.shape {
            Shape::Icon(current) => replace(current, spec),
            _ => false,
        })
    }

    pub fn image(&mut self, texture: TextureId) -> &mut Self {
        if self.swap_texture(texture) {
            self.dirt |= CONTENT;
        }
        self
    }

    pub fn repaint_image(
        &mut self,
        texture: TextureId,
        changed: impl IntoIterator<Item = Rect>,
    ) -> &mut Self {
        if self.before.is_some() && self.swap_texture(texture) {
            self.repainted = true;
            self.scene.mark(self.id, CONTENT);
        }
        self.repaint(changed)
    }

    pub fn repaint(&mut self, changed: impl IntoIterator<Item = Rect>) -> &mut Self {
        let Some(before) = self.before else {
            return self;
        };
        if !self.is_visible() {
            return self;
        }
        for rect in changed
            .into_iter()
            .filter_map(|rect| rect.intersection(&before))
        {
            self.scene.damage.add_for(self.audience, rect);
        }
        self
    }

    pub fn repaint_image_within(
        &mut self,
        texture: TextureId,
        changed: impl IntoIterator<Item = Rect>,
    ) -> &mut Self {
        let offset = self
            .scene
            .get(self.id)
            .map(|node| self.scene.offset_of(node.group));
        let Some(offset) = offset else {
            return self;
        };
        self.repaint_image(
            texture,
            changed.into_iter().map(move |rect| rect.translate(offset)),
        )
    }

    fn swap_texture(&mut self, texture: TextureId) -> bool {
        let id = self.id;
        let Some(node) = self.scene.nodes.get_mut(id.index, id.generation) else {
            return false;
        };
        let Shape::Image {
            texture: current, ..
        } = &mut node.shape
        else {
            return false;
        };
        if *current == texture {
            return false;
        }
        let old = std::mem::replace(current, texture);
        self.scene.replace_texture(id, old, texture);
        true
    }

    pub fn source(&mut self, source: ImageSource) -> &mut Self {
        self.apply(|node| match &mut node.shape {
            Shape::Image {
                source: current, ..
            } => replace(current, source),
            _ => false,
        })
    }

    pub fn backdrop(&mut self, effect: impl PostEffect) -> &mut Self {
        self.set_backdrop(Some(Arc::new(effect)))
    }

    pub fn clear_backdrop(&mut self) -> &mut Self {
        self.set_backdrop(None)
    }

    pub fn backdrop_cache(&mut self, cache: impl Into<Option<BackdropCache>>) -> &mut Self {
        let cache = cache.into();
        if let Some(before) = self.before
            && let Some(cache) = &cache
            && self.is_visible()
        {
            for stale in cache
                .stale
                .iter()
                .filter_map(|stale| stale.intersection(&before))
            {
                self.scene.damage.add_for(self.audience, stale);
            }
        }
        self.touch(KEPT, |node| replace_boxed(&mut node.backdrop_cache, cache))
    }

    fn set_backdrop(&mut self, effect: Option<Arc<dyn PostEffect>>) -> &mut Self {
        let has = effect.is_some();
        let mut had = None;
        self.apply(|node| {
            had = Some(std::mem::replace(&mut node.backdrop, effect).is_some());
            true
        });
        if let Some(had) = had {
            self.scene.count_backdrop(self.id, had, has);
        }
        self
    }

    fn assign<T: PartialEq>(
        &mut self,
        value: T,
        field: impl FnOnce(&mut Node) -> &mut T,
    ) -> &mut Self {
        self.apply(|node| replace(field(node), value))
    }

    fn apply(&mut self, change: impl FnOnce(&mut Node) -> bool) -> &mut Self {
        self.touch(CONTENT, change)
    }

    fn node_mut(&mut self) -> Option<&mut Node> {
        self.before?;
        self.scene.nodes.get_mut(self.id.index, self.id.generation)
    }

    fn repaint_within(
        &mut self,
        changed: Option<impl IntoIterator<Item = Rect>>,
        space: Option<GroupId>,
    ) {
        let Some(changed) = changed else {
            self.dirt |= CONTENT;
            return;
        };
        let offset = self.scene.offset_of(space);
        self.repainted = true;
        self.scene.mark(self.id, CONTENT);
        self.repaint(changed.into_iter().map(|rect| rect.translate(offset)));
    }

    fn touch(&mut self, dirt: Dirt, change: impl FnOnce(&mut Node) -> bool) -> &mut Self {
        if self.before.is_none() {
            return self;
        }
        let id = self.id;
        if let Some(node) = self.scene.nodes.get_mut(id.index, id.generation)
            && change(node)
        {
            self.dirt |= dirt;
        }
        self
    }

    fn is_visible(&self) -> bool {
        self.was_visible || self.scene.get(self.id).is_some_and(|node| node.visible)
    }
}

impl Drop for NodeEdit<'_> {
    fn drop(&mut self) {
        let Some(before) = self.before.filter(|_| self.dirt != 0) else {
            return;
        };
        let id = self.id;
        let Some(node) = self.scene.nodes.get(id.index, id.generation) else {
            return;
        };
        let after = self.scene.bounds_of(node);
        let audience = node.audience;
        let visible_before = self.was_visible;
        let visible_after = node.visible;
        let mut dirt = self.dirt;
        if (visible_before || visible_after) && dirt & (CONTENT | MOVED | SHOWN) != 0 {
            if visible_before {
                self.scene.damage.add_for(self.audience, before);
                if visible_after && audience != self.audience {
                    self.scene.damage.add_for(audience, before);
                }
            }
            if visible_after
                && (!visible_before
                    || (dirt & (CONTENT | MOVED) != 0 && after != before)
                    || audience != self.audience)
            {
                self.scene.damage.add_for(audience, after);
            }
        }
        if dirt & (CONTENT | MOVED) != 0 && after != before {
            dirt |= BOUNDS;
        }
        if dirt & ORDER != 0 {
            self.scene.rerank(id);
        }
        self.scene.mark(id, dirt);
    }
}

impl std::fmt::Debug for NodeEdit<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeEdit")
            .field("id", &self.id)
            .field("changed", &self.changed())
            .finish_non_exhaustive()
    }
}

fn replace<T: PartialEq>(current: &mut T, value: T) -> bool {
    if *current == value {
        return false;
    }
    *current = value;
    true
}

fn replace_boxed<T: PartialEq>(current: &mut Option<Box<T>>, value: Option<T>) -> bool {
    if current.as_deref() == value.as_ref() {
        return false;
    }
    *current = value.map(Box::new);
    true
}
