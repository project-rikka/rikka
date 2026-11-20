use std::sync::Arc;

use rikka_text::{IconSpec, TextPaint};
use rikka_types::{Color, CornerRadii, Lens, Point, Rect, RoundedRect};

use crate::fx::{BackdropCache, Joined, PostEffect, Recipe};
use crate::scene::audience::{Audience, Viewer};
use crate::scene::dirt::Dirt;
use crate::scene::image_source::ImageSource;
use rikka_gpu::texture::TextureId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum GroupClip {
    #[default]
    Inherit,
    Narrow(RoundedRect),
    Only(Option<RoundedRect>),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub struct Stroke {
    pub width: f32,
    pub color: Color,
}

impl Stroke {
    pub fn new(width: f32, color: Color) -> Self {
        Self { width, color }
    }
}

#[derive(Clone, Debug)]
pub enum Shape {
    Rect,
    RoundedRect {
        radii: CornerRadii,
    },
    Circle,
    Arc {
        start_angle: f32,
        sweep_angle: f32,
    },
    Capsule {
        rotation: f32,
    },
    Text(TextPaint),
    Icon(IconSpec),
    Image {
        texture: TextureId,
        source: ImageSource,
        radii: CornerRadii,
        rotation: f32,
    },
}

impl Shape {
    pub(crate) fn fills_interior(&self) -> bool {
        !matches!(self, Shape::Arc { .. })
    }
}

pub(crate) type Rank = (i32, u64, u64, u64);

#[derive(Clone)]
#[must_use]
pub struct Node {
    pub(crate) shape: Shape,
    pub(crate) rect: Rect,
    pub(crate) fill: Color,
    pub(crate) stroke: Option<Stroke>,
    pub(crate) clip: Option<RoundedRect>,
    pub(crate) cutout: Option<Box<RoundedRect>>,
    pub(crate) clip_group: Option<GroupId>,
    pub(crate) z: i32,
    pub(crate) stack: u64,
    pub(crate) label: u64,
    pub(crate) opacity: f32,
    pub(crate) visible: bool,
    pub(crate) audience: Audience,
    pub(crate) clipped_by_group: bool,
    pub(crate) group: Option<GroupId>,
    pub(crate) layer: Option<LayerId>,
    pub(crate) backdrop: Option<Arc<dyn PostEffect>>,
    pub(crate) backdrop_cache: Option<Box<BackdropCache>>,
    pub(crate) lens: Option<Box<Lens>>,
    pub(crate) joined: Option<Box<Joined>>,
    pub(crate) opaque: Option<Rect>,
    pub(crate) seq: u64,
    pub(crate) dirt: Dirt,
}

impl Node {
    fn new(shape: Shape, rect: Rect) -> Self {
        Self {
            shape,
            rect,
            fill: Color::WHITE,
            stroke: None,
            clip: None,
            cutout: None,
            clip_group: None,
            z: 0,
            stack: 0,
            label: 0,
            opacity: 1.0,
            visible: true,
            audience: Audience::Everyone,
            clipped_by_group: true,
            group: None,
            layer: None,
            backdrop: None,
            backdrop_cache: None,
            lens: None,
            joined: None,
            opaque: None,
            seq: 0,
            dirt: 0,
        }
    }

    pub fn rect(rect: Rect) -> Self {
        Self::new(Shape::Rect, rect)
    }

    pub fn rounded_rect(rect: Rect, radii: impl Into<CornerRadii>) -> Self {
        Self::new(
            Shape::RoundedRect {
                radii: radii.into(),
            },
            rect,
        )
    }

    pub fn circle(center: Point, radius: f32) -> Self {
        Self::new(Shape::Circle, Rect::from_circle(center, radius))
    }

    pub fn arc(center: Point, radius: f32, start_angle: f32, sweep_angle: f32) -> Self {
        Self::new(
            Shape::Arc {
                start_angle,
                sweep_angle,
            },
            Rect::from_circle(center, radius),
        )
        .fill(Color::TRANSPARENT)
    }

    pub fn capsule(rect: Rect, rotation: f32) -> Self {
        Self::new(Shape::Capsule { rotation }, rect)
    }

    pub fn text(rect: Rect, paint: impl Into<TextPaint>) -> Self {
        Self::new(Shape::Text(paint.into()), rect)
    }

    pub fn icon(rect: Rect, spec: IconSpec) -> Self {
        Self::new(Shape::Icon(spec), rect)
    }

    pub fn image(rect: Rect, texture: TextureId) -> Self {
        Self::new(
            Shape::Image {
                texture,
                source: ImageSource::whole(),
                radii: CornerRadii::ZERO,
                rotation: 0.0,
            },
            rect,
        )
    }

    pub fn source(mut self, source: ImageSource) -> Self {
        if let Shape::Image {
            source: current, ..
        } = &mut self.shape
        {
            *current = source;
        }
        self
    }

    pub fn radii(mut self, radii: impl Into<CornerRadii>) -> Self {
        if let Shape::RoundedRect { radii: current } | Shape::Image { radii: current, .. } =
            &mut self.shape
        {
            *current = radii.into();
        }
        self
    }

    pub fn rotation(mut self, angle: f32) -> Self {
        if let Shape::Image { rotation, .. } | Shape::Capsule { rotation } = &mut self.shape {
            *rotation = angle;
        }
        self
    }

    pub fn fill(mut self, color: Color) -> Self {
        self.fill = color;
        self
    }

    pub fn stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = Some(stroke);
        self
    }

    pub fn clip(mut self, clip: impl Into<RoundedRect>) -> Self {
        self.clip = Some(clip.into());
        self
    }

    pub fn cut_out(mut self, cutout: impl Into<RoundedRect>) -> Self {
        self.cutout = Some(Box::new(cutout.into()));
        self
    }

    pub fn clip_in(mut self, group: GroupId) -> Self {
        self.clip_group = Some(group);
        self
    }

    pub fn z(mut self, z: i32) -> Self {
        self.z = z;
        self
    }

    pub fn stack(mut self, stack: u64) -> Self {
        self.stack = stack;
        self
    }

    pub fn order(mut self, label: u64) -> Self {
        self.label = label;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn clipped_by_group(mut self, clipped: bool) -> Self {
        self.clipped_by_group = clipped;
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn audience(mut self, audience: Audience) -> Self {
        self.audience = audience;
        self
    }

    pub fn backdrop(mut self, effect: impl PostEffect) -> Self {
        self.backdrop = Some(Arc::new(effect));
        self
    }

    pub fn backdrop_cache(mut self, cache: BackdropCache) -> Self {
        self.backdrop_cache = Some(Box::new(cache));
        self
    }
    pub fn lens(mut self, lens: Lens) -> Self {
        self.lens = Some(Box::new(lens));
        self
    }

    pub fn joined(mut self, joined: Joined) -> Self {
        self.joined = Some(Box::new(joined));
        self
    }

    pub fn opaque_within(mut self, opaque: Rect) -> Self {
        self.opaque = Some(opaque);
        self
    }
    pub fn group(mut self, group: GroupId) -> Self {
        self.group = Some(group);
        self
    }
    pub fn layer(mut self, layer: LayerId) -> Self {
        self.layer = Some(layer);
        self
    }

    pub(crate) fn rank(&self) -> Rank {
        (self.z, self.stack, self.label, self.seq)
    }

    pub(crate) fn paints(&self) -> bool {
        self.visible && self.draws()
    }

    pub(crate) fn seen_by(&self, viewer: Viewer) -> bool {
        self.visible && self.audience.admits(viewer)
    }

    pub(crate) fn draws(&self) -> bool {
        self.opacity > 0.0 && !self.rect.is_empty()
    }

    pub(crate) fn escapes_group_clip(&self) -> bool {
        !self.clipped_by_group && self.group.is_some()
    }

    pub(crate) fn is_textured(&self) -> bool {
        matches!(self.shape, Shape::Image { .. })
    }

    pub(crate) fn recipe(&self, offset: Point) -> Option<Recipe<'_>> {
        let radii = match &self.shape {
            Shape::RoundedRect { radii } => *radii,
            Shape::Circle => CornerRadii::uniform(self.rect.width().min(self.rect.height()) * 0.5),
            _ => CornerRadii::ZERO,
        };
        Some(Recipe {
            effect: self.backdrop.as_deref()?,
            shape: RoundedRect::new(self.rect.translate(offset), radii),
            lens: self.lens.as_deref().copied(),
            joined: self
                .joined
                .as_deref()
                .map(|joined| joined.translate(offset)),
        })
    }

    pub(crate) fn paint_bounds(&self, offset: Point, clip_offset: Point) -> Rect {
        let bounds = match &self.shape {
            Shape::Text(paint) => self.rect.inflate(paint.layout.font_size() * paint.scale),
            Shape::Icon(spec) => self.rect.inflate(spec.size * 0.25),
            Shape::Image { rotation, .. } | Shape::Capsule { rotation } if *rotation != 0.0 => {
                rotated_bounds(self.rect, *rotation)
            }
            _ => self.rect,
        }
        .translate(offset);
        match self.clip {
            Some(clip) => bounds
                .intersection(&clip.rect.translate(clip_offset))
                .unwrap_or(Rect::ZERO),
            None => bounds,
        }
    }
}
const TURNED_EDGE: f32 = 1.0;

fn rotated_bounds(rect: Rect, rotation: f32) -> Rect {
    let (sin, cos) = rotation.sin_cos();
    let reach_x = (rect.width() * cos.abs() + rect.height() * sin.abs()) / 2.0 + TURNED_EDGE;
    let reach_y = (rect.width() * sin.abs() + rect.height() * cos.abs()) / 2.0 + TURNED_EDGE;
    let center = rect.center();
    Rect::new(
        center.x - reach_x,
        center.y - reach_y,
        reach_x * 2.0,
        reach_y * 2.0,
    )
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node")
            .field("shape", &self.shape)
            .field("rect", &self.rect)
            .field("z", &self.z)
            .field("stack", &self.stack)
            .field("order", &self.label)
            .field("visible", &self.visible)
            .field("audience", &self.audience)
            .finish_non_exhaustive()
    }
}
