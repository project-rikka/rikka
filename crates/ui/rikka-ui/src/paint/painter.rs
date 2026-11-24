use std::hash::Hash;
use std::sync::Arc;
use std::time::Instant;

use rikka_animation::{Curve, Motion};
use rikka_renderer::{Blur, Transform};
use rikka_text::{ColorScheme, IconSpec, TextLayout, TextPaint};
use rikka_theme::{Palette, Theme, Translucency};
use rikka_types::{Color, CornerRadii, Lens, Point, Rect, RoundedRect, Size, WindowView};

use crate::icon_source::IconSource;
use crate::motion::{Animatable, Clock, Motions, POPPED};
use crate::paint::revealing::Revealing;
use crate::paint::sprite::Sprite;
use crate::picture::ImageSource;
use crate::scroll::Reveal;
use crate::text::TextRun;
use crate::view::{Env, hash_of};

const INVISIBLE_ALPHA: f32 = 0.5 / 255.0;

const PLACE_GRID: f32 = 1024.0;

pub(crate) fn on_grid(value: f32) -> f32 {
    (value * PLACE_GRID).round() / PLACE_GRID
}

pub(crate) fn settled(rect: Rect) -> Rect {
    Rect::new(
        on_grid(rect.x()),
        on_grid(rect.y()),
        rect.width(),
        rect.height(),
    )
}

pub(crate) const READS_CHILDREN: u8 = 1;
pub(crate) const READS_CLIP: u8 = 1 << 1;
pub(crate) const READS_VIEWPORT: u8 = 1 << 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClipKind {
    Base,
    Local,
    Volatile,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SceneClip {
    pub(crate) shape: Option<RoundedRect>,
    pub(crate) kind: ClipKind,
    pub(crate) section: Option<u32>,
}

impl SceneClip {
    pub(crate) const fn base(shape: Option<RoundedRect>) -> Self {
        SceneClip {
            shape,
            kind: ClipKind::Base,
            section: None,
        }
    }

    pub(crate) fn volatile(&self) -> bool {
        self.shape.is_some() && self.kind == ClipKind::Volatile
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PaintedPrimitive {
    pub(crate) prim: Primitive,
    pub(crate) fade: Option<usize>,
    pub(crate) clip: ClipKind,
    pub(crate) scene: SceneClip,
    pub(crate) topmost: bool,
    pub(crate) section: Option<u32>,
    pub(crate) part: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Section {
    pub(crate) by: Point,
    pub(crate) shown: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Fade {
    pub(crate) within: Option<usize>,
    pub(crate) opacity: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Primitive {
    Quad {
        rect: Rect,
        fill: Color,
        radii: CornerRadii,
        stroke: Option<(f32, Color)>,
        blur: Option<f32>,
        lens: Option<Lens>,
        cutout: Option<RoundedRect>,
        clip: Option<RoundedRect>,
        opacity: f32,
    },
    Arc {
        center: Point,
        radius: f32,
        start_angle: f32,
        sweep_angle: f32,
        width: f32,
        color: Color,
        clip: Option<RoundedRect>,
    },
    Text {
        rect: Rect,
        run: TextRun,
        paint: TextPaint,
        color: Color,
        clip: Option<RoundedRect>,
    },
    Icon {
        rect: Rect,
        spec: IconSpec,
        scale: f32,
        color: Color,
        clip: Option<RoundedRect>,
    },
    Image {
        rect: Rect,
        source: ImageSource,
        placeholder: Option<ImageSource>,
        tint: Color,
        pixelated: bool,
        radii: CornerRadii,
        turn: f32,
        mirror: Transform,
        clip: Option<RoundedRect>,
    },
    Segment {
        rect: Rect,
        rotation: f32,
        color: Color,
        clip: Option<RoundedRect>,
    },
}

impl Primitive {
    pub(crate) fn translate(&mut self, origin: Point) {
        if origin == Point::ZERO {
            return;
        }
        let back = Point::new(-origin.x, -origin.y);
        match self {
            Primitive::Quad { rect, .. }
            | Primitive::Text { rect, .. }
            | Primitive::Icon { rect, .. }
            | Primitive::Image { rect, .. }
            | Primitive::Segment { rect, .. } => *rect = rect.translate(back),
            Primitive::Arc { center, .. } => *center += back,
        }
    }

    pub(crate) fn place(&mut self, origin: Point, clip_origin: Point) {
        self.translate(origin);
        if clip_origin != Point::ZERO {
            self.move_clips(Point::new(-clip_origin.x, -clip_origin.y));
        }
        self.settle();
    }

    fn settle(&mut self) {
        let shape = |shape: &mut Option<RoundedRect>| {
            if let Some(shape) = shape {
                shape.rect = settled(shape.rect);
            }
        };
        match self {
            Primitive::Quad {
                rect, clip, cutout, ..
            } => {
                *rect = settled(*rect);
                shape(clip);
                shape(cutout);
            }
            Primitive::Text { rect, clip, .. }
            | Primitive::Icon { rect, clip, .. }
            | Primitive::Image { rect, clip, .. }
            | Primitive::Segment { rect, clip, .. } => {
                *rect = settled(*rect);
                shape(clip);
            }
            Primitive::Arc { center, clip, .. } => {
                *center = Point::new(on_grid(center.x), on_grid(center.y));
                shape(clip);
            }
        }
    }

    pub(crate) fn unplaced(mut self, origin: Point, clip_origin: Point) -> Self {
        self.translate(Point::new(-origin.x, -origin.y));
        if clip_origin != Point::ZERO {
            self.move_clips(clip_origin);
        }
        self
    }

    pub(crate) fn shifted(mut self, by: Point) -> Self {
        if by == Point::ZERO {
            return self;
        }
        self.translate(Point::new(-by.x, -by.y));
        self.move_clips(by);
        self
    }

    pub(crate) fn clip(&self) -> Option<RoundedRect> {
        match self {
            Primitive::Quad { clip, .. }
            | Primitive::Arc { clip, .. }
            | Primitive::Text { clip, .. }
            | Primitive::Icon { clip, .. }
            | Primitive::Image { clip, .. }
            | Primitive::Segment { clip, .. } => *clip,
        }
    }

    pub(crate) fn with_clip(mut self, shape: Option<RoundedRect>) -> Self {
        self.set_clip(shape);
        self
    }

    pub(crate) fn set_clip(&mut self, shape: Option<RoundedRect>) {
        match self {
            Primitive::Quad { clip, .. }
            | Primitive::Arc { clip, .. }
            | Primitive::Text { clip, .. }
            | Primitive::Icon { clip, .. }
            | Primitive::Image { clip, .. }
            | Primitive::Segment { clip, .. } => *clip = shape,
        }
    }

    pub(crate) fn has_cutout(&self) -> bool {
        matches!(
            self,
            Primitive::Quad {
                cutout: Some(_),
                ..
            }
        )
    }

    fn move_clips(&mut self, by: Point) {
        let moved = |shape: &mut Option<RoundedRect>| {
            if let Some(shape) = shape {
                shape.rect = shape.rect.translate(by);
            }
        };
        match self {
            Primitive::Quad { clip, cutout, .. } => {
                moved(clip);
                moved(cutout);
            }
            Primitive::Arc { clip, .. }
            | Primitive::Text { clip, .. }
            | Primitive::Icon { clip, .. }
            | Primitive::Image { clip, .. }
            | Primitive::Segment { clip, .. } => moved(clip),
        }
    }
    fn fades_by(&mut self, opacity: f32) -> bool {
        match self {
            Primitive::Quad { opacity: alpha, .. } => *alpha *= opacity,
            Primitive::Arc { color, .. }
            | Primitive::Icon { color, .. }
            | Primitive::Segment { color, .. } => {
                *color = color.scale_alpha(opacity);
            }
            Primitive::Text { run, .. } if !run.spans.is_empty() => return false,
            Primitive::Text { color, .. } => *color = color.scale_alpha(opacity),
            Primitive::Image { tint, .. } => *tint = tint.scale_alpha(opacity),
        }
        true
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Quad {
    rect: Rect,
    fill: Color,
    radii: CornerRadii,
    stroke: Option<(f32, Color)>,
    blur: Option<f32>,
    cutout: Option<RoundedRect>,
}

impl Quad {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            fill: Color::TRANSPARENT,
            radii: CornerRadii::ZERO,
            stroke: None,
            blur: None,
            cutout: None,
        }
    }

    pub fn fill(mut self, color: Color) -> Self {
        self.fill = color;
        self
    }

    pub fn radius(mut self, radii: impl Into<CornerRadii>) -> Self {
        self.radii = radii.into();
        self
    }

    pub fn stroke(mut self, width: f32, color: Color) -> Self {
        self.stroke = Some((width, color));
        self
    }

    pub fn blur(mut self, sigma: f32) -> Self {
        self.blur = Some(sigma);
        self
    }

    pub fn cut_out(mut self, shape: impl Into<RoundedRect>) -> Self {
        self.cutout = Some(shape.into());
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ring {
    center: Point,
    diameter: f32,
    width: f32,
    color: Color,
    start_angle: f32,
    sweep_angle: f32,
}

impl Ring {
    pub const TOP: f32 = -std::f32::consts::FRAC_PI_2;

    pub fn new(center: Point, diameter: f32) -> Self {
        Self {
            center,
            diameter,
            width: 0.0,
            color: Color::TRANSPARENT,
            start_angle: 0.0,
            sweep_angle: std::f32::consts::TAU,
        }
    }

    pub fn inside(bounds: Rect, ratio: f32) -> (Self, f32) {
        let diameter = bounds.size.min_side();
        (
            Self::new(bounds.center(), diameter),
            (diameter * ratio).max(1.0),
        )
    }

    pub fn stroke(mut self, width: f32, color: Color) -> Self {
        self.width = width;
        self.color = color;
        self
    }

    pub fn sweep(mut self, start: f32, sweep: f32) -> Self {
        self.start_angle = start;
        self.sweep_angle = sweep;
        self
    }
}

pub struct PaintCtx<'a> {
    pub(crate) bounds: Rect,
    pub(crate) room: Rect,
    pub(crate) content_size: Size,
    pub(crate) viewport: Size,
    pub(crate) layouts: &'a crate::runtime::Layouts,
    pub(crate) node: crate::runtime::LayoutNode,
    pub(crate) theme: &'a crate::ambient::ThemeHandle,
    pub(crate) env: &'a Env,
    pub(crate) scale: f32,
    pub(crate) offset: Point,
    pub(crate) clip: Option<RoundedRect>,
    pub(crate) hovered: bool,
    pub(crate) focused: bool,
    pub(crate) focus_within: bool,
    pub(crate) clock: Clock,
    pub(crate) animating: &'a mut bool,
    pub(crate) wake: &'a mut Option<Instant>,
    pub(crate) motions: &'a mut Motions,
    pub(crate) reveal: &'a mut Option<Revealing>,
    pub(crate) subtree_opacity: f32,
    pub(crate) subtree_scale: Option<(f32, Point)>,
    pub(crate) children_clip: Option<RoundedRect>,
    pub(crate) fades: &'a mut Vec<Fade>,
    pub(crate) fade: Option<usize>,
    pub(crate) out: &'a mut Vec<PaintedPrimitive>,
    pub(crate) rebuild: &'a mut bool,
    pub(crate) build: &'a mut bool,
    pub(crate) prefetch: &'a mut bool,
    pub(crate) glass: &'a mut Vec<RoundedRect>,
    pub(crate) opaque: &'a mut Vec<RoundedRect>,
    pub(crate) windows: &'a mut Vec<WindowView>,
    pub(crate) pulses: &'a mut Vec<rikka_picture::Pulse>,
    pub(crate) grounded: &'a mut bool,
    pub(crate) clip_kind: ClipKind,
    pub(crate) scene_clip: SceneClip,
    pub(crate) reads: std::cell::Cell<u8>,
    pub(crate) instance: u64,
    pub(crate) marks: &'a Arc<crate::runtime::Marks>,
    pub(crate) motion_scope: Option<u64>,
    pub(crate) topmost: bool,
    pub(crate) sections: &'a mut Vec<Section>,
    pub(crate) section: Option<u32>,
    pub(crate) section_by: Point,
    pub(crate) part: Option<u64>,
}

impl std::fmt::Debug for PaintCtx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaintCtx")
            .field("bounds", &self.bounds)
            .field("content_size", &self.content_size)
            .field("scale", &self.scale)
            .field("hovered", &self.hovered)
            .field("focused", &self.focused)
            .finish_non_exhaustive()
    }
}

impl PaintCtx<'_> {
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    pub fn room(&self) -> Rect {
        self.room
    }

    pub fn content_size(&self) -> Size {
        self.content_size
    }

    pub fn visible(&self) -> Rect {
        self.read(READS_CLIP);
        match self.clip {
            Some(clip) => clip.rect.intersection(&self.bounds).unwrap_or(Rect::ZERO),
            None => self.bounds,
        }
    }

    pub fn viewport(&self) -> Size {
        self.read(READS_VIEWPORT);
        self.viewport
    }

    pub fn child_rect(&self, index: usize) -> Option<Rect> {
        self.read(READS_CHILDREN);
        let rect = self.layouts.child_rect(self.node, index)?;
        Some(rect.translate(self.bounds.origin))
    }

    pub fn placed(&self) -> Rect {
        self.layouts.rect(self.node)
    }

    pub fn handle(&self) -> crate::runtime::NodeHandle {
        crate::runtime::NodeHandle::new(self.instance, self.marks)
    }

    fn read(&self, what: u8) {
        self.reads.set(self.reads.get() | what);
    }

    pub fn palette(&self) -> Palette {
        self.theme().palette()
    }

    pub fn theme(&self) -> &Theme {
        self.theme
    }

    pub fn env(&self) -> &Env {
        self.env
    }

    pub fn hovered(&self) -> bool {
        self.hovered
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    pub fn focus_within(&self) -> bool {
        self.focus_within
    }

    pub fn now(&self) -> Instant {
        self.clock.now
    }

    pub fn request_frame(&mut self) {
        *self.animating = true;
    }

    pub fn pulse(&mut self, pulse: &rikka_picture::Pulse) {
        rikka_picture::Pulse::join(self.pulses, pulse.clone());
    }

    pub fn wake_at(&mut self, at: Instant) {
        *self.wake = Some(self.wake.map_or(at, |soonest| soonest.min(at)));
    }

    pub fn request_view(&mut self) {
        *self.rebuild = true;
    }

    pub fn request_build(&mut self) {
        *self.build = true;
    }

    pub fn request_prefetch(&mut self) {
        *self.prefetch = true;
    }

    pub fn request_layout(&mut self) {
        self.handle().relayout();
    }

    pub fn reveal(&mut self, rect: Rect, reveal: Reveal) {
        *self.reveal = Some(Revealing::new(rect, reveal, self.node));
    }

    pub fn reveal_with_parent_start(&mut self, rect: Rect) {
        *self.reveal = Some(Revealing::new(rect, Reveal::Nearest, self.node).with_parent_start());
    }

    pub fn animate<T: Animatable>(&mut self, key: impl Hash, target: T, motion: Motion) -> T {
        self.animate_curve(key, target, motion.curve(T::PROPERTY))
    }

    pub fn animate_curve<T: Animatable>(&mut self, key: impl Hash, target: T, curve: Curve) -> T {
        self.drive(self.motion_key(key), target, target, curve)
    }

    pub fn steer<T: Animatable>(&mut self, key: impl Hash, target: T, grabbed: bool) -> T {
        let curve = match grabbed {
            true => crate::motion::DRAG,
            false => crate::motion::VALUE,
        };
        self.animate_curve(key, target, curve)
    }

    pub fn fade(&mut self, key: impl Hash, target: Option<Color>, curve: Curve) -> Option<Color> {
        let key = self.motion_key(key);
        let target = match target {
            Some(color) => color,
            None => self.motions.target::<Color>(key)?.with_alpha(0.0),
        };
        let color = self.drive(key, target, target, curve);
        (color.a > INVISIBLE_ALPHA).then_some(color)
    }

    pub fn hold<T: Animatable>(
        &mut self,
        key: impl Hash,
        target: Option<T>,
        curve: Curve,
    ) -> Option<T> {
        let key = self.motion_key(key);
        let target = match target {
            Some(target) => target,
            None => self.motions.target::<T>(key)?,
        };
        Some(self.drive(key, target, target, curve))
    }

    pub fn progress(&mut self, key: impl Hash, toward: bool, enter: Curve, exit: Curve) -> f32 {
        let curve = match toward {
            true => enter,
            false => exit,
        };
        let key = self.motion_key(key);
        let (progress, moving) = self.motions.present(key, toward, curve, self.clock);
        if moving {
            self.request_frame();
        }
        progress
    }

    pub fn play(&mut self, key: impl Hash, curve: Curve) -> f32 {
        self.drive(self.motion_key(key), 0.0, 1.0, curve)
    }

    pub fn jump<T: Animatable>(&mut self, key: impl Hash, value: T) {
        self.motions.jump(self.motion_key(key), value, self.clock);
    }

    pub fn fling<T: Animatable>(&mut self, key: impl Hash, value: T, velocity: T) {
        self.motions
            .fling(self.motion_key(key), value, velocity, self.clock);
        self.request_frame();
    }

    pub fn impel<T: Animatable>(&mut self, key: impl Hash, velocity: T) {
        self.motions
            .impel(self.motion_key(key), velocity, self.clock);
    }

    pub fn shift<T: Animatable>(&mut self, key: impl Hash, by: T) {
        self.motions.shift(self.motion_key(key), by);
    }

    pub fn restart(&mut self, key: impl Hash) {
        self.motions.forget(self.motion_key(key));
    }

    pub fn is_animating(&self, key: impl Hash) -> bool {
        self.motions.holds(self.motion_key(key))
    }

    pub fn motion_scope<R>(&mut self, scope: impl Hash, draw: impl FnOnce(&mut Self) -> R) -> R {
        let inner = self.motion_key(scope);
        let outer = self.motion_scope.replace(inner);
        let drawn = draw(self);
        self.motion_scope = outer;
        drawn
    }

    fn motion_key(&self, key: impl Hash) -> u64 {
        match self.motion_scope {
            Some(scope) => hash_of((scope, key)),
            None => hash_of(key),
        }
    }

    fn drive<T: Animatable>(&mut self, key: u64, start: T, target: T, curve: Curve) -> T {
        let (value, moving) = self.motions.drive(key, start, target, curve, self.clock);
        if moving {
            self.request_frame();
        }
        value
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn snap(&self, value: f32) -> f32 {
        (value * self.scale).round() / self.scale
    }

    pub fn clipped(&mut self, shape: impl Into<RoundedRect>, draw: impl FnOnce(&mut Self)) {
        let shape = shape.into();
        let narrowed = match self.clip {
            Some(outer) => narrowed(
                (outer, self.clip_kind),
                (shape, ClipKind::Local),
                ClipKind::Volatile,
            ),
            None => Some((shape, ClipKind::Local)),
        };
        let Some((clip, kind)) = narrowed else {
            self.read(READS_CLIP);
            return;
        };
        let scene = narrowed_scene(
            self.scene_clip,
            RoundedRect::new(shape.rect.translate(self.section_by), shape.radii),
            self.section,
        );
        let previous = self.clip.replace(clip);
        let previous_kind = std::mem::replace(&mut self.clip_kind, kind);
        let standing_scene = std::mem::replace(&mut self.scene_clip, scene);
        draw(self);
        self.clip = previous;
        self.clip_kind = previous_kind;
        self.scene_clip = standing_scene;
    }

    pub fn scrolled(&mut self, by: Point, draw: impl FnOnce(&mut Self)) {
        let total = self.section_by + by;
        let shown = self.place(self.bounds.translate(self.section_by));
        let index = self.sections.len() as u32;
        self.sections.push(Section {
            by: total * self.scale,
            shown,
        });
        let back = Point::ZERO - by;
        let clip = self
            .clip
            .map(|clip| RoundedRect::new(clip.rect.translate(back), clip.radii));
        let previous = std::mem::replace(&mut self.clip, clip);
        let section = self.section.replace(index);
        let section_by = std::mem::replace(&mut self.section_by, total);
        draw(self);
        self.clip = previous;
        self.section = section;
        self.section_by = section_by;
    }

    pub fn part(&mut self, key: impl Hash, draw: impl FnOnce(&mut Self)) {
        let inner = hash_of((self.part, key));
        let outer = self.part.replace(inner);
        draw(self);
        self.part = outer;
    }

    pub fn topmost(&mut self, draw: impl FnOnce(&mut Self)) {
        let clip = self.clip.take();
        let kind = std::mem::replace(&mut self.clip_kind, ClipKind::Local);
        let scene = std::mem::replace(
            &mut self.scene_clip,
            SceneClip {
                shape: None,
                kind: ClipKind::Local,
                section: None,
            },
        );
        let topmost = std::mem::replace(&mut self.topmost, true);
        draw(self);
        self.clip = clip;
        self.clip_kind = kind;
        self.scene_clip = scene;
        self.topmost = topmost;
    }

    pub fn with_opacity(&mut self, opacity: f32, draw: impl FnOnce(&mut Self)) {
        let opacity = opacity.clamp(0.0, 1.0);
        if opacity >= 1.0 {
            draw(self);
            return;
        }
        let drawn = self.out.len();
        let previous = self.fade;
        self.fades.push(Fade {
            within: previous,
            opacity,
        });
        self.fade = Some(self.fades.len() - 1);
        draw(self);
        self.fade = previous;

        let alone = match self.out.len() - drawn {
            0 => true,
            1 => {
                let painted = &mut self.out[drawn];
                let took = painted.prim.fades_by(opacity);
                if took {
                    painted.fade = previous;
                }
                took
            }
            _ => false,
        };
        if alone {
            self.fades.pop();
        }
    }

    pub fn zoomed(&mut self, factor: f32, pivot: Point, draw: impl FnOnce(&mut Self)) {
        if factor == 1.0 {
            draw(self);
            return;
        }
        if factor <= 0.0 {
            return;
        }
        let unzoomed = |shape: RoundedRect, pivot: Point| {
            let origin = (shape.rect.origin - pivot * (1.0 - factor)) * (1.0 / factor);
            RoundedRect::new(
                Rect::from_origin_size(origin, shape.rect.size * (1.0 / factor)),
                shape.radii * (1.0 / factor),
            )
        };
        let clip = self.clip.map(|clip| unzoomed(clip, pivot));
        let scene = self.scene_clip.shape.map(|shape| unzoomed(shape, pivot));
        let clip = std::mem::replace(&mut self.clip, clip);
        let scene = std::mem::replace(&mut self.scene_clip.shape, scene);
        let section_by = self.section_by;
        self.section_by = section_by * (1.0 / factor);
        let scale = self.scale;
        let offset = self.offset;
        self.offset = offset + pivot * scale * (1.0 - factor);
        self.scale = scale * factor;
        draw(self);
        self.scale = scale;
        self.offset = offset;
        self.section_by = section_by;
        self.clip = clip;
        self.scene_clip.shape = scene;
    }

    pub fn popped(&mut self, shown: f32, pivot: Point, draw: impl FnOnce(&mut Self)) {
        if shown <= 0.0 {
            return;
        }
        let pose = POPPED.toward_rest(shown.min(1.0), Size::ZERO);
        self.with_opacity(pose.opacity, |ctx| ctx.zoomed(pose.scale, pivot, draw));
    }

    pub fn pop_subtree(&mut self, shown: f32, pivot: Point) {
        let pose = POPPED.toward_rest(shown.clamp(0.0, 1.0), Size::ZERO);
        self.set_subtree_opacity(pose.opacity);
        if pose.scale != 1.0 {
            self.set_subtree_scale(pose.scale, pivot);
        }
    }

    pub fn set_subtree_opacity(&mut self, opacity: f32) {
        self.subtree_opacity = opacity.clamp(0.0, 1.0);
    }

    pub fn set_subtree_scale(&mut self, factor: f32, pivot: Point) {
        self.subtree_scale = Some((factor.max(0.0), pivot));
    }

    pub fn set_children_clip(&mut self, clip: impl Into<RoundedRect>) {
        self.children_clip = Some(clip.into());
    }

    pub fn draw_window(&mut self, view: WindowView) {
        let rect = self.place(view.rect);
        let own = view.clip.map(|clip| self.place_shape(clip));
        let outer = self.physical_clip();
        if outer.is_some() {
            self.read(READS_CLIP);
        }
        let clip = match (own, outer) {
            (Some(own), Some(outer)) => match own.intersect(outer) {
                Some(both) => Some(both),
                None => return,
            },
            (own, outer) => own.or(outer),
        };
        let opacity = view.opacity * self.faded();
        self.windows.push(WindowView {
            rect,
            clip,
            opacity,
            ..view
        });
    }

    fn faded(&self) -> f32 {
        std::iter::successors(self.fade, |at| self.fades[*at].within)
            .map(|at| self.fades[at].opacity)
            .product()
    }

    pub fn glass(&mut self, shape: impl Into<RoundedRect>) {
        *self.grounded = true;
        self.pane(shape.into());
    }

    pub fn opaque(&mut self, shape: impl Into<RoundedRect>) {
        if self.faded() < 1.0 {
            return;
        }
        let shape: RoundedRect = shape.into();
        let shape = RoundedRect::new(shape.rect, shape.radii.clamp_for_size(shape.rect.size));
        let placed = self.place_shape(shape);
        let placed = match self.physical_clip() {
            Some(clip) => {
                self.read(READS_CLIP);
                match placed.intersect(clip) {
                    Some(both) => both,
                    None => return,
                }
            }
            None => placed,
        };
        self.opaque.push(placed);
    }

    fn pane(&mut self, shape: RoundedRect) {
        let shape = RoundedRect::new(shape.rect, shape.radii.clamp_for_size(shape.rect.size));
        self.glass.push(self.place_shape(shape));
    }

    pub fn draw_pane(&mut self, quad: Quad) {
        let shape = RoundedRect::new(quad.rect, quad.radii);
        let frost = self
            .theme
            .translucency()
            .blur_radius()
            .map(|radius| Blur::with_radius(radius).sigma());
        self.draw_quad(Quad {
            blur: frost,
            ..quad
        });
        self.pane(shape);
    }

    pub fn draw_scrim(&mut self, rect: Rect) {
        let frost = Blur::with_radius(Translucency::SCRIM_BLUR).sigma();
        self.push_quad(Quad::new(rect).blur(frost), None);
    }

    pub fn draw_quad(&mut self, quad: Quad) {
        let lens = quad.blur.and(self.lens());
        self.push_quad(quad, lens);
    }

    fn push_quad(&mut self, quad: Quad, lens: Option<Lens>) {
        let scale = self.scale;
        let frost = lens.map_or(1.0, Lens::frost);
        self.push(Primitive::Quad {
            rect: self.place(quad.rect),
            fill: quad.fill,
            radii: quad.radii * scale,
            stroke: quad.stroke.map(|(w, c)| (w * scale, c)),
            blur: quad.blur.map(|sigma| sigma * scale * frost),
            lens,
            cutout: quad.cutout.map(|shape| self.place_shape(shape)),
            clip: self.physical_clip(),
            opacity: 1.0,
        });
    }

    pub fn draw_circle(&mut self, center: Point, diameter: f32, fill: Color) {
        self.draw_quad(
            Quad::new(Rect::from_circle(center, diameter / 2.0))
                .fill(fill)
                .radius(diameter / 2.0),
        );
    }

    pub fn draw_segment(&mut self, from: Point, to: Point, width: f32, color: Color) {
        let length = from.distance(to);
        let center = Point::new((from.x + to.x) * 0.5, (from.y + to.y) * 0.5);
        let rect = Rect::new(
            center.x - (length + width) * 0.5,
            center.y - width * 0.5,
            length + width,
            width,
        );
        self.push(Primitive::Segment {
            rect: self.place(rect),
            rotation: (to.y - from.y).atan2(to.x - from.x),
            color,
            clip: self.physical_clip(),
        });
    }

    pub fn draw_ring(&mut self, ring: Ring) {
        let scale = self.scale;
        self.push(Primitive::Arc {
            center: self.place_point(ring.center),
            radius: ring.diameter / 2.0 * scale,
            start_angle: ring.start_angle,
            sweep_angle: ring.sweep_angle,
            width: ring.width * scale,
            color: ring.color,
            clip: self.physical_clip(),
        });
    }

    pub fn draw_text(&mut self, rect: Rect, run: TextRun, color: Color) {
        let width = rect.width();
        let laid = self
            .layouts
            .leaf(self.node)
            .and_then(|leaf| leaf.laid(&run, width))
            .unwrap_or_else(|| rikka_text::layout_approx(run.as_shape(), Some(width)));
        self.draw_laid_text(rect, run, laid, color);
    }

    pub(crate) fn draw_measured_text(&mut self, rect: Rect, color: Color) -> bool {
        let Some(leaf) = self.layouts.leaf(self.node) else {
            return false;
        };
        let Some(run) = leaf.measured_run() else {
            return false;
        };
        let solved = self.layouts.solved_width(self.node);
        let width = match (solved - rect.width()).abs() < 1.0 {
            true => solved,
            false => rect.width(),
        };
        let laid = leaf
            .laid_within(width)
            .unwrap_or_else(|| rikka_text::layout_approx(run.as_shape(), Some(width)));
        let run = run.clone();
        self.draw_laid_text(rect, run, laid, color);
        true
    }

    pub fn draw_laid_text(
        &mut self,
        rect: Rect,
        run: TextRun,
        laid: Arc<TextLayout>,
        color: Color,
    ) {
        let scheme = match self.theme().is_dark() {
            true => ColorScheme::Dark,
            false => ColorScheme::Light,
        };
        let paint = TextPaint::new(laid)
            .with_spans(&run.spans)
            .scaled(self.scale)
            .with_color_scheme(scheme);
        let run = crate::text::elide::shown(run, &paint.layout);
        self.push(Primitive::Text {
            rect: self.place(rect),
            run,
            paint,
            color,
            clip: self.physical_clip(),
        });
    }

    pub fn draw_text_line(&mut self, within: Rect, x: f32, width: f32, run: TextRun, color: Color) {
        let rect = Rect::new(
            x,
            within.center().y - run.line_height / 2.0,
            width.max(0.0),
            run.line_height,
        );
        self.draw_text(rect, run, color);
    }

    pub fn draw_icon(&mut self, rect: Rect, spec: IconSpec, color: Color) {
        self.push(Primitive::Icon {
            rect: self.place(rect),
            spec,
            scale: self.scale,
            color,
            clip: self.physical_clip(),
        });
    }

    pub fn draw_image(&mut self, rect: Rect, source: impl Into<ImageSource>) {
        self.draw_sprite(Sprite::new(rect, source));
    }

    pub fn draw_image_pixelated(&mut self, rect: Rect, source: impl Into<ImageSource>) {
        self.draw_sprite(Sprite::new(rect, source).pixelated());
    }

    pub fn draw_mask(&mut self, rect: Rect, source: impl Into<ImageSource>, color: Color) {
        self.draw_sprite(Sprite::new(rect, source).tinted(color));
    }

    pub fn draw_sprite(&mut self, sprite: Sprite) {
        let rect = self.place(sprite.rect);
        self.push(Primitive::Image {
            source: sprite.source.rastered_for(rect.size),
            placeholder: sprite
                .placeholder
                .map(|placeholder| placeholder.rastered_for(rect.size)),
            rect,
            tint: sprite.tint,
            pixelated: sprite.pixelated,
            radii: sprite.radii * self.scale,
            turn: sprite.turn,
            mirror: sprite.mirror,
            clip: self.physical_clip(),
        });
    }

    pub fn draw_icon_source(&mut self, rect: Rect, source: &IconSource, size: f32, color: Color) {
        match source {
            IconSource::Glyph(glyph) => self.draw_icon(rect, glyph.spec(size), color),
            IconSource::Picture(picture) => self.draw_image(rect, picture.clone()),
            IconSource::Mask(picture) => self.draw_mask(rect, picture.clone(), color),
        }
    }

    fn lens(&self) -> Option<Lens> {
        Some(self.theme.translucency().lens()?.scaled(self.scale))
    }

    fn place(&self, rect: Rect) -> Rect {
        rect.scale(self.scale).translate(self.offset)
    }

    fn place_point(&self, point: Point) -> Point {
        point * self.scale + self.offset
    }

    fn physical_clip(&self) -> Option<RoundedRect> {
        self.clip.map(|clip| self.place_shape(clip))
    }

    fn place_shape(&self, shape: RoundedRect) -> RoundedRect {
        RoundedRect::new(self.place(shape.rect), shape.radii * self.scale)
    }

    fn push(&mut self, prim: Primitive) {
        let scene = SceneClip {
            shape: self.scene_clip.shape.map(|shape| self.place_shape(shape)),
            ..self.scene_clip
        };
        self.out.push(PaintedPrimitive {
            prim,
            fade: self.fade,
            clip: self.clip_kind,
            scene,
            topmost: self.topmost,
            section: self.section,
            part: self.part,
        });
    }
}

pub(crate) fn narrowed<K: Copy>(
    outer: (RoundedRect, K),
    inner: (RoundedRect, K),
    volatile: K,
) -> Option<(RoundedRect, K)> {
    let shape = inner.0.intersect(outer.0)?;
    let kind = match shape {
        _ if shape == inner.0 => inner.1,
        _ if shape == outer.0 => outer.1,
        _ => volatile,
    };
    Some((shape, kind))
}
fn narrowed_scene(outer: SceneClip, shape: RoundedRect, section: Option<u32>) -> SceneClip {
    let Some(bound) = outer.shape else {
        return SceneClip {
            shape: Some(shape),
            kind: ClipKind::Local,
            section,
        };
    };
    let Some(both) = shape.intersect(bound) else {
        return SceneClip {
            shape: Some(RoundedRect::rect(Rect::ZERO)),
            kind: ClipKind::Volatile,
            section: None,
        };
    };
    let (kind, section) = match both {
        _ if both == shape => (ClipKind::Local, section),
        _ if both == bound => (outer.kind, outer.section),
        _ => (ClipKind::Volatile, None),
    };
    SceneClip {
        shape: Some(both),
        kind,
        section,
    }
}
