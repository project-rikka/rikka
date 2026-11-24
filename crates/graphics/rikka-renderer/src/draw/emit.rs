use rikka_sync::Mutex;
use rikka_text::{GlyphKind, IconKey, LayoutRow, PlacedGlyph, TextPaint};
use rikka_types::{Color, CornerRadii, Rect};

use crate::draw::instance::{
    ExtraSlots, FLAG_CIRCLE, FLAG_LENS, FLAG_PIXELATED, FLAG_SHADOW, FLAG_UV_SWAP, GpuInstance,
    KIND_BACKDROP, KIND_GLYPH_COLOR, KIND_GLYPH_CURVES, KIND_GLYPH_MASK, KIND_SHAPE, KIND_TEXTURED,
};
use crate::scene::{Node, Scene, Shape};
use crate::text::atlas::{AtlasKey, GlyphAtlas, GlyphRef, GlyphSlot, ImageSlot};
use crate::text::curve_store::CurveSlot;
use crate::text::routing::{GlyphRoute, GlyphRouting};
use rikka_gpu::texture::TextureBook;

const CURVE_MARGIN: f32 = 1.0;

const AHEAD_REACH: f32 = 384.0;

const KEPT_REACH: f32 = 256.0;

const INSIDE: f32 = 0.5;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Band {
    from: f32,
    to: f32,
}

impl Band {
    fn reaches(self, row: &LayoutRow<'_>, scale: f32) -> bool {
        row.bottom() >= self.from / scale && row.top() < self.to / scale
    }

    fn reaches_every_row(self, paint: &TextPaint) -> bool {
        let last_top =
            paint.layout.row_count().saturating_sub(1) as f32 * paint.layout.line_height();
        self.from <= 0.0 && last_top < self.to / paint.scale
    }

    pub(crate) fn covers(self, shown: Band) -> bool {
        shown.from >= shown.to || (self.from <= shown.from && shown.to <= self.to)
    }
}

pub(crate) fn shown_band(scene: &Scene, node: &Node) -> Band {
    band_within(scene, node, 0.0)
}

pub(crate) fn ahead_band(scene: &Scene, node: &Node) -> Band {
    band_within(scene, node, AHEAD_REACH)
}

pub(crate) fn resident_band(
    scene: &Scene,
    node: &Node,
    shown: Band,
    routing: GlyphRouting,
    rows: &mut Vec<(f32, f32, bool)>,
    atlas: &Mutex<GlyphAtlas>,
) -> Band {
    let Shape::Text(paint) = &node.shape else {
        return shown;
    };
    if shown.reaches_every_row(paint) {
        return shown;
    }
    let scale = paint.scale;
    rows.clear();
    let atlas = atlas.lock();
    for (row, baseline) in placed_rows(paint, node, band_within(scene, node, KEPT_REACH)) {
        if shown.reaches(&row, scale) {
            continue;
        }
        let resident = row
            .placed((node.rect.min_x(), baseline), scale)
            .all(|glyph| atlas.holds(&routing.route(glyph.key, paint.color_scheme, false).key));
        rows.push((row.top() * scale, row.bottom() * scale, resident));
    }
    drop(atlas);
    let (mut from, mut to) = (shown.from, shown.to);
    let below = rows.iter().filter(|(top, _, _)| *top >= shown.to);
    for &(_, bottom, resident) in below {
        if !resident {
            break;
        }
        to = bottom;
    }
    let above = rows
        .iter()
        .rev()
        .filter(|(_, bottom, _)| *bottom <= shown.from);
    for &(top, _, resident) in above {
        if !resident {
            break;
        }
        from = top + INSIDE;
    }
    Band {
        from: from.min(shown.from),
        to: to.max(shown.to),
    }
}

fn band_within(scene: &Scene, node: &Node, reach: f32) -> Band {
    if !matches!(node.shape, Shape::Text(_)) {
        return Band::default();
    }
    let height = node.rect.height();
    let own = node.clip.map(|clip| {
        let top = match node.clip_group == node.group {
            true => node.rect.min_y(),
            false => {
                node.rect.min_y() + scene.offset_of(node.group).y
                    - scene.offset_of(node.clip_group).y
            }
        };
        (clip.rect, top)
    });
    let grouped = scene
        .group_clip_of(node)
        .map(|clip| (clip.rect, node.rect.min_y() + scene.offset_of(node.group).y));
    let (from, to) =
        [own, grouped]
            .into_iter()
            .flatten()
            .fold((0.0_f32, height), |(from, to), (clip, top)| {
                (
                    (clip.min_y() - reach - top).max(from),
                    (clip.max_y() + reach - top).min(to),
                )
            });
    Band {
        from,
        to: to.max(from),
    }
}

pub(crate) fn glass_instance(node: &Node, clips: ExtraSlots) -> GpuInstance {
    let glass = shape_instance(node, KIND_BACKDROP, clips);
    match node.lens {
        Some(_) => glass.with_flags(FLAG_LENS),
        None => glass,
    }
}

pub(crate) fn collect_keys(
    keys: &mut Vec<GlyphRoute>,
    node: &Node,
    band: Band,
    routing: GlyphRouting,
    zooming: bool,
) {
    match &node.shape {
        Shape::Text(paint) => {
            for (row, baseline) in placed_rows(paint, node, band) {
                let placed = row.placed((node.rect.min_x(), baseline), paint.scale);
                keys.extend(
                    placed.map(|glyph| routing.route(glyph.key, paint.color_scheme, zooming)),
                );
            }
        }
        Shape::Icon(spec) => keys.push(GlyphRoute::unscaled(AtlasKey::Icon(IconKey::from(spec)))),
        _ => {}
    }
}

pub(crate) fn collect_ahead_keys(
    keys: &mut Vec<AtlasKey>,
    node: &Node,
    prefetched: [Band; 2],
    ahead: Band,
    routing: GlyphRouting,
) {
    let Shape::Text(paint) = &node.shape else {
        return;
    };
    if prefetched[0].reaches_every_row(paint) {
        return;
    }
    let scale = paint.scale;
    for (row, baseline) in placed_rows(paint, node, ahead) {
        if prefetched.iter().any(|band| band.reaches(&row, scale)) {
            continue;
        }
        let placed = row.placed((node.rect.min_x(), baseline), scale);
        keys.extend(placed.map(|glyph| routing.route(glyph.key, paint.color_scheme, false).key));
    }
}

fn placed_rows<'a>(
    paint: &'a TextPaint,
    node: &Node,
    band: Band,
) -> impl Iterator<Item = (LayoutRow<'a>, f32)> {
    let scale = paint.scale;
    let top = node.rect.min_y().round();
    paint
        .layout
        .rows_between(band.from / scale, band.to / scale)
        .map(move |row| {
            let snap = (row.top() * scale).round() - row.top() * scale;
            let baseline = top + row.baseline() * scale + snap;
            (row, baseline)
        })
}

pub(crate) fn whole_shift(node: &Node, band: Band, from_x: f32) -> Option<f32> {
    let Shape::Text(paint) = &node.shape else {
        return None;
    };
    let moved = node.rect.min_x() - from_x;
    let exact = moved == moved.round();
    let mut shift = None;
    for (row, baseline) in placed_rows(paint, node, band) {
        if !exact && !row.decorations().is_empty() {
            return None;
        }
        let was = row.placed((from_x, baseline), paint.scale);
        let now = row.placed((node.rect.min_x(), baseline), paint.scale);
        for (was, now) in was.zip(now) {
            if was.key != now.key {
                return None;
            }
            let by = now.x - was.x;
            match shift {
                None => shift = Some(by),
                Some(owner) if owner != by => return None,
                Some(_) => {}
            }
        }
    }
    match shift {
        Some(by) => Some(by as f32),
        None if exact => Some(moved),
        None => Some(0.0),
    }
}

fn apply_stroke(instance: GpuInstance, node: &Node) -> GpuInstance {
    match node.stroke {
        Some(stroke) => instance.with_stroke(stroke.width, stroke.color, node.opacity),
        None => instance,
    }
}

pub(crate) fn emit_node(
    instances: &mut Vec<GpuInstance>,
    node: &Node,
    book: &TextureBook,
    glyphs: &[Option<GlyphRef>],
    band: Band,
    clips: ExtraSlots,
) {
    match &node.shape {
        Shape::Rect
        | Shape::RoundedRect { .. }
        | Shape::Circle
        | Shape::Arc { .. }
        | Shape::Capsule { .. }
        | Shape::Shadow { .. } => {
            instances.push(shape_instance(node, KIND_SHAPE, clips));
        }
        Shape::Image {
            texture,
            source,
            radii,
            rotation,
        } => {
            let Some(listing) = book.get(*texture) else {
                tracing::warn!("image node references stale texture {texture:?}");
                return;
            };
            let (tex, extent) = (listing.slot, listing.extent);
            if source.minifies(extent, node.rect.size) {
                book.drawn_minified(*texture, &listing);
            }
            let (from, to, swapped) = source.sample(extent);
            let instance = GpuInstance::new(KIND_TEXTURED, node.rect)
                .with_fill(node.fill, node.opacity)
                .with_texture(tex, from, to)
                .with_color(source.color.slot())
                .with_radii(radii.clamp_for_size(node.rect.size))
                .with_turn(*rotation)
                .placed(node, clips)
                .with_flags(if swapped { FLAG_UV_SWAP } else { 0 })
                .with_flags(if source.pixelated { FLAG_PIXELATED } else { 0 });
            instances.push(apply_stroke(instance, node));
        }
        Shape::Text(paint) => {
            let placed = GpuInstance::new(KIND_GLYPH_MASK, Rect::ZERO).placed(node, clips);
            let mut slots = glyphs.iter();
            for (row, baseline) in placed_rows(paint, node, band) {
                for glyph in row.placed((node.rect.min_x(), baseline), paint.scale) {
                    let Some(Some(GlyphRef {
                        slot: Some(slot), ..
                    })) = slots.next()
                    else {
                        continue;
                    };
                    let color = paint.ink(glyph.offset).unwrap_or(node.fill);
                    instances.push(match *slot {
                        GlyphSlot::Image(image) => glyph_instance(
                            &placed,
                            image,
                            (glyph.x as f32, glyph.y as f32),
                            color,
                            node.opacity,
                        ),
                        GlyphSlot::Curves(curves) => {
                            curve_instance(&placed, curves, &glyph, color, node.opacity)
                        }
                    });
                }
                emit_decorations(instances, &placed, node, paint, &row, baseline);
            }
        }
        Shape::Icon(spec) => {
            let Some(Some(GlyphRef {
                slot: Some(GlyphSlot::Image(slot)),
                ..
            })) = glyphs.first()
            else {
                return;
            };
            let middle = node.rect.center();
            let corner = (
                (middle.x - spec.size / 2.0).round(),
                (middle.y + spec.size / 2.0).round(),
            );
            let placed = GpuInstance::new(KIND_GLYPH_MASK, Rect::ZERO).placed(node, clips);
            instances.push(glyph_instance(
                &placed,
                *slot,
                corner,
                node.fill,
                node.opacity,
            ));
        }
    }
}

fn emit_decorations(
    instances: &mut Vec<GpuInstance>,
    placed: &GpuInstance,
    node: &Node,
    paint: &TextPaint,
    row: &LayoutRow<'_>,
    baseline: f32,
) {
    let scale = paint.scale;
    let left_edge = node.rect.min_x();
    for decoration in row.decorations() {
        if decoration.width <= 0.0 {
            continue;
        }
        let offset = row.line_start() + decoration.start as usize;
        let color = paint.ink(Some(offset)).unwrap_or(node.fill);
        let thickness = (decoration.thickness * scale).max(1.0).ceil();
        let mut instance = *placed;
        instance.bounds = [
            left_edge + decoration.x * scale,
            baseline + decoration.y * scale,
            decoration.width * scale,
            thickness,
        ];
        instance.set_kind(KIND_SHAPE);
        instances.push(instance.with_fill(color, node.opacity));
    }
}

fn glyph_instance(
    placed: &GpuInstance,
    slot: ImageSlot,
    pen: (f32, f32),
    mask_color: Color,
    opacity: f32,
) -> GpuInstance {
    let scale = slot.scale;
    let mut instance = *placed;
    instance.bounds = [
        pen.0 + slot.left as f32 * scale,
        pen.1 - slot.top as f32 * scale,
        slot.width as f32 * scale,
        slot.height as f32 * scale,
    ];
    let (kind, fill) = match slot.kind {
        GlyphKind::Mask => (KIND_GLYPH_MASK, mask_color),
        GlyphKind::Color => (KIND_GLYPH_COLOR, Color::WHITE),
    };
    instance.set_kind(kind);
    instance
        .with_fill(fill, opacity)
        .with_texture(slot.tex, slot.uv_min, slot.uv_max)
}

fn curve_instance(
    placed: &GpuInstance,
    slot: CurveSlot,
    glyph: &PlacedGlyph,
    color: Color,
    opacity: f32,
) -> GpuInstance {
    let key = glyph.key;
    let pixels_per_em = key.size();
    let origin = (glyph.x as f32 + key.subpixel_x(), glyph.y as f32);
    let [x_min, y_min, x_max, y_max] = slot.bounds;
    let left = (origin.0 + x_min * pixels_per_em).floor() - CURVE_MARGIN;
    let right = (origin.0 + x_max * pixels_per_em).ceil() + CURVE_MARGIN;
    let top = (origin.1 - y_max * pixels_per_em).floor() - CURVE_MARGIN;
    let bottom = (origin.1 - y_min * pixels_per_em).ceil() + CURVE_MARGIN;
    let em = |x: f32, y: f32| {
        [
            (x - origin.0) / pixels_per_em,
            (origin.1 - y) / pixels_per_em,
        ]
    };
    let mut instance = *placed;
    instance.bounds = [left, top, right - left, bottom - top];
    instance.set_kind(KIND_GLYPH_CURVES);
    instance
        .with_fill(color, opacity)
        .with_texture(slot.offset, em(left, top), em(right, bottom))
        .with_curves(pixels_per_em)
}

fn shape_instance(node: &Node, kind: u32, clips: ExtraSlots) -> GpuInstance {
    let instance = GpuInstance::new(kind, node.rect)
        .with_fill(node.fill, node.opacity)
        .placed(node, clips);
    let shaped = with_geometry(instance, node);
    match node.shape {
        Shape::Shadow { .. } => shaped,
        _ => apply_stroke(shaped, node),
    }
}

fn with_geometry(instance: GpuInstance, node: &Node) -> GpuInstance {
    match &node.shape {
        Shape::RoundedRect { radii } => instance.with_radii(radii.clamp_for_size(node.rect.size)),
        Shape::Circle => instance.with_flags(FLAG_CIRCLE),
        Shape::Arc {
            start_angle,
            sweep_angle,
        } => instance.with_arc(*start_angle, *sweep_angle),
        Shape::Capsule { rotation } => instance
            .with_radii(CornerRadii::uniform(node.rect.height() * 0.5))
            .with_turn(*rotation),
        Shape::Shadow { radii, sigma } => {
            let mut shadow = instance.with_radii(*radii).with_flags(FLAG_SHADOW);
            shadow.stroke_width = *sigma;
            shadow
        }
        _ => instance,
    }
}
