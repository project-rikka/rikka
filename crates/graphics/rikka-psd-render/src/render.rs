use std::borrow::Cow;

use rikka_psd::{
    BlendMode, BlendRange, ChannelId, Document, Effects, FillContent, Group, Knockout, Layer,
    LayerKind, LayerMask, Node, Plane, Rect, ResourceData, Result, resource_id,
};

use crate::adjust::Operation;
use crate::blend::blend_adjusted;
use crate::composite::{Fade, composite_faded, composite_raster};
use crate::effects::{self, EffectInputs, Light};
use crate::mask::{Mask, coverage_row};
use crate::paint::PaintContext;
use crate::planar::Planar;
use crate::raster::{Raster, lerp_row};
use crate::space::Space;
use crate::vector;

const BAND: usize = 64;

const DEFAULT_GLOBAL_ANGLE: i32 = 120;

const DEFAULT_GLOBAL_ALTITUDE: i32 = 30;

pub struct Renderer<'a> {
    document: &'a Document,
    paint: PaintContext<'a>,
    light: Light,
    effects_visible: bool,
}

impl std::fmt::Debug for Renderer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Renderer")
            .field("space", &self.paint.space)
            .finish_non_exhaustive()
    }
}

struct Backdrops<'s> {
    initial: &'s Raster,
    deep: &'s Raster,
    adjustment_clip: Option<&'s Mask>,
}

struct Content {
    raster: Raster,
    vector_consumed: bool,
}

impl<'a> Renderer<'a> {
    pub fn new(document: &'a Document) -> Self {
        let space = Space::from_document(document);
        let light = Light {
            angle: f64::from(
                document
                    .resources
                    .global_angle()
                    .unwrap_or(DEFAULT_GLOBAL_ANGLE),
            ),
            altitude: f64::from(
                document
                    .resources
                    .global_altitude()
                    .unwrap_or(DEFAULT_GLOBAL_ALTITUDE),
            ),
        };
        let effects_visible = !matches!(
            document.resources.get(resource_id::EFFECTS_VISIBLE),
            Some(ResourceData::EffectsVisible(false))
        );
        Self {
            document,
            paint: PaintContext::new(document, space),
            light,
            effects_visible,
        }
    }
    pub(crate) fn space(&self) -> Space {
        self.paint.space
    }

    pub(crate) fn bounds(&self) -> Rect {
        self.document.bounds()
    }

    pub fn render(&self) -> Result<Raster> {
        self.render_region(self.bounds())
    }

    pub(crate) fn render_region(&self, area: Rect) -> Result<Raster> {
        if self.document.layers.is_empty() {
            return self.render_composite(area);
        }
        self.render_nodes(&self.document.layers, area)
    }

    fn render_composite(&self, area: Rect) -> Result<Raster> {
        let space = self.paint.space;
        let document = self.document;
        let mut raster = Raster::new(area, space.channels());
        let visible = area.intersection(&self.bounds());
        if document.color_mode == rikka_psd::ColorMode::Indexed {
            let image = document.composite_rgba()?;
            let mut rgb = [0.0f32; 4];
            for y in visible.top..visible.bottom {
                for x in visible.left..visible.right {
                    let [r, g, b, a] = image.get(x as u32, y as u32);
                    space.encode_rgb([r, g, b], &mut rgb);
                    let index = raster.index(x, y).expect("pixel inside area");
                    for (slot, value) in raster.data[index..index + space.channels()]
                        .iter_mut()
                        .zip(&rgb)
                    {
                        *slot = value * a;
                    }
                    raster.data[index + space.channels()] = a;
                }
            }
            return Ok(raster);
        }
        let normalized = |index: usize| -> Result<Option<Vec<f32>>> {
            Ok(document
                .composite_plane(index)?
                .map(|plane| plane.to_normalized()))
        };
        let planes = (0..space.channels())
            .map(normalized)
            .collect::<Result<Vec<_>>>()?;
        let alpha = match document.composite_alpha_index() {
            Some(index) => normalized(index)?,
            None => None,
        };
        let alpha = alpha.as_ref();
        let white = space.white();
        let width = document.width as usize;
        for y in visible.top..visible.bottom {
            for x in visible.left..visible.right {
                let source = y as usize * width + x as usize;
                let a = alpha.map_or(1.0, |alpha| alpha[source]);
                let index = raster.index(x, y).expect("pixel inside area");
                for (channel, matte) in white.iter().enumerate().take(space.channels()) {
                    let stored = planes[channel].as_ref().map_or(0.0, |plane| plane[source]);
                    raster.data[index + channel] = if alpha.is_some() {
                        (stored - matte * (1.0 - a)).max(0.0)
                    } else {
                        stored
                    };
                }
                raster.data[index + space.channels()] = a;
            }
        }
        Ok(raster)
    }

    pub(crate) fn render_nodes(&self, nodes: &[Node], area: Rect) -> Result<Raster> {
        let mut canvas = Raster::new(area, self.paint.space.channels());
        let empty = Raster::new(area, self.paint.space.channels());
        let background = match nodes.first() {
            Some(Node::Layer(layer))
                if layer.visible()
                    && layer.channel(ChannelId::Transparency).is_none()
                    && !layer.bounds.is_empty() =>
            {
                let mut backdrop = empty.clone();
                self.draw_node(
                    &nodes[0],
                    &[],
                    &mut backdrop,
                    &Backdrops {
                        initial: &empty,
                        deep: &empty,
                        adjustment_clip: None,
                    },
                )?;
                Some(backdrop)
            }
            _ => None,
        };
        self.draw_nodes(
            nodes,
            &mut canvas,
            &Backdrops {
                initial: &empty,
                deep: background.as_ref().unwrap_or(&empty),
                adjustment_clip: None,
            },
        )?;
        Ok(canvas)
    }

    fn draw_nodes(
        &self,
        nodes: &[Node],
        canvas: &mut Raster,
        backdrops: &Backdrops<'_>,
    ) -> Result<()> {
        let mut index = 0;
        while index < nodes.len() {
            let mut end = index + 1;
            while end < nodes.len() && nodes[end].layer().clipping {
                end += 1;
            }
            self.draw_node(&nodes[index], &nodes[index + 1..end], canvas, backdrops)?;
            index = end;
        }
        Ok(())
    }

    fn draw_node(
        &self,
        node: &Node,
        clips: &[Node],
        canvas: &mut Raster,
        backdrops: &Backdrops<'_>,
    ) -> Result<()> {
        let layer = node.layer();
        if !layer.visible() {
            return Ok(());
        }
        match node {
            Node::Group(group) => self.draw_group(group, clips, canvas, backdrops),
            Node::Layer(layer) => {
                if layer.kind() == LayerKind::Adjustment {
                    self.draw_adjustment(layer, canvas, backdrops.adjustment_clip)?;
                    return self.draw_nodes_unclipped(clips, canvas, backdrops);
                }
                let Some(content) = self.content(layer)? else {
                    return Ok(());
                };
                self.draw_styled(layer, layer.blend_mode, content, clips, canvas, backdrops)
            }
        }
    }

    fn draw_nodes_unclipped(
        &self,
        nodes: &[Node],
        canvas: &mut Raster,
        backdrops: &Backdrops<'_>,
    ) -> Result<()> {
        for node in nodes {
            self.draw_node(node, &[], canvas, backdrops)?;
        }
        Ok(())
    }

    fn draw_group(
        &self,
        group: &Group,
        clips: &[Node],
        canvas: &mut Raster,
        backdrops: &Backdrops<'_>,
    ) -> Result<()> {
        let layer = &group.layer;
        let mode = group.blend_mode();
        let styled = self.effects(layer).is_some();
        let artboard = artboard(layer, &self.paint.space);
        if mode == BlendMode::PassThrough
            && !styled
            && artboard.is_none()
            && layer.knockout() == Knockout::None
        {
            let fill = layer.fill_opacity();
            let opacity = layer.opacity() * fill;
            let global = self.combined_mask(layer, canvas.bounds)?;
            if opacity >= 1.0 && clips.is_empty() && global.is_none() {
                return self.draw_through(group, canvas, backdrops);
            }
            let mut target = canvas.clone();
            let isolated_shape = if fill < 1.0 || !clips.is_empty() {
                let initial = Raster::new(canvas.bounds, canvas.channels);
                let mut children_raster = Raster::new(canvas.bounds, canvas.channels);
                self.draw_nodes(
                    &group.children,
                    &mut children_raster,
                    &Backdrops {
                        initial: &initial,
                        deep: backdrops.deep,
                        adjustment_clip: None,
                    },
                )?;
                Some(children_raster.alpha_mask())
            } else {
                None
            };
            let clip = match (isolated_shape.as_ref(), backdrops.adjustment_clip) {
                (Some(shape), Some(outer)) => {
                    let mut combined = shape.clone();
                    combined.multiply(outer);
                    Some(Cow::Owned(combined))
                }
                (Some(shape), None) => Some(Cow::Borrowed(shape)),
                (None, Some(outer)) => Some(Cow::Borrowed(outer)),
                (None, None) => None,
            };
            self.draw_nodes(
                &group.children,
                &mut target,
                &Backdrops {
                    initial: canvas,
                    deep: backdrops.deep,
                    adjustment_clip: clip.as_deref(),
                },
            )?;
            if let Some(shape) = &isolated_shape {
                for node in clips {
                    self.draw_clipped(node, &mut target, shape, backdrops)?;
                }
            }
            canvas.lerp_from(&target, opacity, global.as_slice());
            return Ok(());
        }
        let mut isolated = Raster::new(canvas.bounds, canvas.channels);
        if let Some((rect, Some(color))) = &artboard {
            let mut fill = Mask::new(canvas.bounds, 0.0);
            for y in rect.top..rect.bottom {
                for x in rect.left..rect.right {
                    fill.set(x, y, 1.0);
                }
            }
            crate::composite::composite_color(
                &mut isolated,
                &fill,
                *color,
                BlendMode::Normal,
                &self.paint.space,
                1.0,
            );
        }
        let initial = isolated.clone();
        self.draw_nodes(
            &group.children,
            &mut isolated,
            &Backdrops {
                initial: &initial,
                deep: if mode == BlendMode::PassThrough {
                    backdrops.deep
                } else {
                    &initial
                },
                adjustment_clip: None,
            },
        )?;
        if let Some((rect, _)) = &artboard {
            let inside = *rect;
            let mut clip = Mask::new(isolated.bounds, 0.0);
            for y in inside.top..inside.bottom {
                for x in inside.left..inside.right {
                    clip.set(x, y, 1.0);
                }
            }
            isolated.multiply_alpha(&clip);
        }
        let mode = if mode == BlendMode::PassThrough {
            BlendMode::Normal
        } else {
            mode
        };
        self.draw_styled(
            layer,
            mode,
            Content {
                raster: isolated,
                vector_consumed: false,
            },
            clips,
            canvas,
            backdrops,
        )
    }

    fn draw_through(
        &self,
        group: &Group,
        canvas: &mut Raster,
        backdrops: &Backdrops<'_>,
    ) -> Result<()> {
        let before = group
            .children
            .iter()
            .any(|node| node.layer().knockout() == Knockout::Shallow)
            .then(|| canvas.clone());
        self.draw_nodes(
            &group.children,
            canvas,
            &Backdrops {
                initial: before.as_ref().unwrap_or(backdrops.initial),
                deep: backdrops.deep,
                adjustment_clip: backdrops.adjustment_clip,
            },
        )
    }

    fn effects(&self, layer: &Layer) -> Option<Effects> {
        if !self.effects_visible {
            return None;
        }
        layer.effects().filter(Effects::has_visible)
    }

    fn content(&self, layer: &Layer) -> Result<Option<Content>> {
        let fill = layer.fill();
        let has_pixels = !layer.bounds.is_empty() && layer.channel(ChannelId::Color(0)).is_some();
        if let Some(fill) = fill
            && !has_pixels
        {
            return Ok(Some(self.synthesize(layer, &fill)));
        }
        if !has_pixels {
            return Ok(None);
        }
        Ok(Some(Content {
            raster: self.decode(layer)?,
            vector_consumed: layer
                .vector_stroke()
                .is_some_and(|stroke| stroke.stroke_enabled),
        }))
    }

    fn decode(&self, layer: &Layer) -> Result<Raster> {
        let space = self.paint.space;
        let channels = space.channels();
        let depth = self.document.depth;
        let planes = (0..channels)
            .map(|index| layer.decode_channel(ChannelId::Color(index as u16), depth))
            .collect::<Result<Vec<_>>>()?;
        let transparency = layer.decode_channel(ChannelId::Transparency, depth)?;
        let fallback = if space.kind == crate::space::SpaceKind::Cmyk {
            1.0
        } else {
            0.0
        };
        let mut raster = Raster::new(layer.bounds, channels);
        let stride = raster.stride();
        let row = layer.bounds.width() as usize * stride;
        if row == 0 {
            return Ok(raster);
        }
        for (top, pixels) in (0..).step_by(BAND).zip(raster.data.chunks_mut(BAND * row)) {
            let rows = top..top + (pixels.len() / row) as u32;
            let count = pixels.len() / stride;
            let normalized = |plane: &Option<Plane>, value: f32| match plane {
                Some(plane) => plane.normalized_rows(rows.clone()),
                None => vec![value; count],
            };
            let alpha = normalized(&transparency, 1.0);
            let mut color: [Vec<f32>; 4] = Default::default();
            for (slot, plane) in color.iter_mut().zip(&planes) {
                *slot = normalized(plane, fallback);
                if transparency.is_some() {
                    for (value, alpha) in slot.iter_mut().zip(&alpha) {
                        *value *= alpha;
                    }
                }
            }
            Planar { color, alpha }.join(pixels, channels);
        }
        Ok(raster)
    }

    fn synthesize(&self, layer: &Layer, fill: &FillContent) -> Content {
        let document_bounds = self.bounds();
        let vector = layer.vector_mask().filter(|mask| !mask.disabled());
        let stroke = layer.vector_stroke();
        let reference = vector
            .and_then(|mask| {
                let coverage = vector::rasterize(
                    &mask.path,
                    self.document.width,
                    self.document.height,
                    document_bounds,
                );
                mask_coverage(&coverage)
            })
            .or_else(|| (!layer.bounds.is_empty()).then_some(layer.bounds))
            .or_else(|| {
                layer
                    .mask
                    .filter(|mask| mask.default_color == 0 && !mask.bounds.is_empty())
                    .map(|mask| mask.bounds)
            })
            .unwrap_or(document_bounds);
        let reach = stroke
            .as_ref()
            .map_or(0, |stroke| effects::margin(stroke.width));
        let area = document_bounds.expanded(reach);
        let mut raster = Raster::new(area, self.paint.space.channels());
        let Some(vector) = vector else {
            let painted = self.paint.paint(fill, area, reference);
            composite_raster(
                &mut raster,
                &painted,
                BlendMode::Normal,
                &self.paint.space,
                None,
                1.0,
            );
            return Content {
                raster,
                vector_consumed: false,
            };
        };
        let mut coverage = vector::rasterize(
            &vector.path,
            self.document.width,
            self.document.height,
            area,
        );
        if vector.inverted() {
            coverage = coverage.inverted();
        }
        let fill_enabled = stroke.as_ref().is_none_or(|stroke| stroke.fill_enabled);
        if fill_enabled {
            let painted = self.paint.paint(fill, area, reference);
            composite_raster(
                &mut raster,
                &painted,
                BlendMode::Normal,
                &self.paint.space,
                Some(&coverage),
                1.0,
            );
        }
        if let Some(stroke) = stroke.filter(|stroke| stroke.stroke_enabled && stroke.width > 0.0) {
            let (wide, tall) = (self.document.width, self.document.height);
            let width = stroke.width as f32;
            let mut band = match stroke.alignment {
                rikka_psd::LineAlignment::Center => {
                    vector::stroke(&vector.path, wide, tall, &stroke, width, area)
                }
                rikka_psd::LineAlignment::Inside => {
                    let mut band =
                        vector::stroke(&vector.path, wide, tall, &stroke, width * 2.0, area);
                    band.multiply(&coverage);
                    band
                }
                rikka_psd::LineAlignment::Outside => {
                    let mut band =
                        vector::stroke(&vector.path, wide, tall, &stroke, width * 2.0, area);
                    band.multiply(&coverage.inverted());
                    band
                }
            };
            band.map(|value| value.clamp(0.0, 1.0));
            if let Some(content) = &stroke.content {
                let painted = self.paint.paint(content, area, reference);
                composite_raster(
                    &mut raster,
                    &painted,
                    stroke.blend_mode,
                    &self.paint.space,
                    Some(&band),
                    stroke.opacity as f32,
                );
            }
        }
        Content {
            raster,
            vector_consumed: true,
        }
    }

    fn user_mask(&self, mask: &LayerMask, layer: &Layer) -> Result<Option<Mask>> {
        let derived = mask.flags.from_other_data() && layer.vector_mask().is_some();
        let (bounds, default_color, flags, channel) = if derived {
            match mask.real {
                Some(real) if layer.channel(ChannelId::RealUserMask).is_some() => (
                    real.bounds,
                    real.default_color,
                    real.flags,
                    ChannelId::RealUserMask,
                ),
                _ => return Ok(None),
            }
        } else {
            (
                mask.bounds,
                mask.default_color,
                mask.flags,
                ChannelId::UserMask,
            )
        };
        if flags.disabled() {
            return Ok(None);
        }
        let outside = f32::from(default_color) / 255.0;
        let mut result = Mask::filled(bounds, outside, outside);
        if let Some(plane) = layer.decode_channel(channel, self.document.depth)? {
            result.data = plane.to_normalized();
        }
        if flags.inverted() {
            result = result.inverted();
        }
        if let Some(density) = mask.parameters.user_density {
            let density = f32::from(density) / 255.0;
            result.map(|value| 1.0 - density * (1.0 - value));
        }
        if let Some(feather) = mask
            .parameters
            .user_feather
            .filter(|feather| *feather > 0.0)
        {
            let grown = result.resized(bounds.expanded((feather * 3.0).ceil() as i32));
            result = grown.blurred(feather as f32);
        }
        Ok(Some(result))
    }

    fn vector_mask(&self, layer: &Layer, area: Rect) -> Option<Mask> {
        let vector = layer.vector_mask().filter(|mask| !mask.disabled())?;
        let mut mask = vector::rasterize(
            &vector.path,
            self.document.width,
            self.document.height,
            area,
        );
        if vector.inverted() {
            mask = mask.inverted();
        }
        let parameters = layer.mask.map(|mask| mask.parameters).unwrap_or_default();
        if let Some(feather) = parameters.vector_feather.filter(|feather| *feather > 0.0) {
            mask = mask.blurred(feather as f32);
        }
        if let Some(density) = parameters.vector_density {
            let density = f32::from(density) / 255.0;
            mask.map(|value| 1.0 - density * (1.0 - value));
        }
        Some(mask)
    }

    fn combined_mask(&self, layer: &Layer, area: Rect) -> Result<Option<Mask>> {
        let user = match &layer.mask {
            Some(mask) => self.user_mask(mask, layer)?,
            None => None,
        };
        let vector = self.vector_mask(layer, area);
        Ok(match (user, vector) {
            (None, None) => None,
            (Some(user), None) => Some(user),
            (None, Some(vector)) => Some(vector),
            (Some(user), Some(mut vector)) => {
                vector.multiply(&user);
                Some(vector)
            }
        })
    }

    fn draw_adjustment(
        &self,
        layer: &Layer,
        canvas: &mut Raster,
        clip: Option<&Mask>,
    ) -> Result<()> {
        let Some(operation) = Operation::from_layer(layer) else {
            return Ok(());
        };
        let space = self.paint.space;
        let bounds = canvas.bounds;
        if bounds.is_empty() {
            return Ok(());
        }
        let global = self.combined_mask(layer, bounds)?;
        let masks: Vec<&Mask> = global.iter().chain(clip).collect();
        let amount = layer.opacity() * layer.fill_opacity();
        let columns = bounds.left..bounds.right;
        let has_blend_mode =
            !matches!(layer.blend_mode, BlendMode::Normal | BlendMode::PassThrough);
        let adjuster = operation.adjuster(space);
        let mut factors = vec![0.0; columns.len()];
        let mut adjusted = vec![0.0; columns.len() * canvas.stride()];
        for y in bounds.top..bounds.bottom {
            coverage_row(amount, &masks, y, bounds.left, &mut factors);
            if factors.iter().all(|&factor| factor <= 0.0) {
                continue;
            }
            let span = canvas.span(y, columns.clone());
            let row = &mut canvas.data[span];
            adjusted.copy_from_slice(row);
            adjuster.apply(&mut adjusted);
            if has_blend_mode {
                blend_adjusted(&mut adjusted, row, layer.blend_mode, &space);
            }
            lerp_row(row, &adjusted, &factors, canvas.channels + 1);
        }
        Ok(())
    }

    fn draw_styled(
        &self,
        layer: &Layer,
        mode: BlendMode,
        content: Content,
        clips: &[Node],
        canvas: &mut Raster,
        backdrops: &Backdrops<'_>,
    ) -> Result<()> {
        let space = self.paint.space;
        let effects = self.effects(layer);
        let reach = effects.as_ref().map_or(0, effects::reach);
        let content_bounds = raster_coverage(&content.raster).unwrap_or(Rect::EMPTY);
        let area = content_bounds
            .expanded(reach)
            .intersection(&canvas.bounds.expanded(reach + 2));
        if area.is_empty() {
            return Ok(());
        }
        let user = match &layer.mask {
            Some(mask) => self.user_mask(mask, layer)?,
            None => None,
        };
        let vector = if content.vector_consumed {
            None
        } else {
            self.vector_mask(layer, area)
        };
        let user_global = layer.layer_mask_hides_effects();
        let vector_global = layer.vector_mask_hides_effects();
        let mut masked = content.raster;
        let mut shape_masks = Vec::new();
        let mut global_masks = Vec::new();
        for (mask, global) in [(user, user_global), (vector, vector_global)] {
            if let Some(mask) = mask {
                if global {
                    global_masks.push(mask)
                } else {
                    shape_masks.push(mask)
                }
            }
        }
        for mask in &shape_masks {
            masked.multiply_alpha(mask);
        }
        let restricted = restricted_channels(layer, &space);
        if effects.is_none()
            && clips.is_empty()
            && restricted.is_empty()
            && layer.knockout() == Knockout::None
            && !layer.blend_interior_effects()
            && layer.blending_ranges.is_neutral()
        {
            let fade = Fade {
                opacity: layer.opacity(),
                masks: &global_masks,
            };
            composite_faded(canvas, &masked, mode, &space, layer.fill_opacity(), fade);
            return Ok(());
        }
        let shape = masked.alpha_mask().resized(area);
        if !layer.blending_ranges.is_neutral() {
            let weights = self.blend_if(layer, &masked, canvas, area);
            masked.multiply_alpha(&weights);
        }
        let faded = layer.opacity() < 1.0 || !global_masks.is_empty() || !restricted.is_empty();
        let mut region = faded.then(|| canvas.region(area));
        let target = match &mut region {
            Some(region) => region,
            None => &mut *canvas,
        };
        let inputs = EffectInputs::new(&shape, content_bounds, &self.paint, self.light);
        if let Some(effects) = &effects {
            effects::outer(effects, &inputs, target);
        }
        match layer.knockout() {
            Knockout::None => {}
            Knockout::Shallow => target.lerp_from(backdrops.initial, 1.0, &[&shape]),
            Knockout::Deep => target.lerp_from(backdrops.deep, 1.0, &[&shape]),
        }
        let before = (!restricted.is_empty()).then(|| target.clone());
        let fill = layer.fill_opacity();
        if !clips.is_empty() || layer.blend_interior_effects() {
            let mut grouped = Raster::new(area, space.channels());
            let interior_inside = layer.blend_interior_effects();
            composite_raster(
                &mut grouped,
                &masked,
                BlendMode::Normal,
                &space,
                None,
                if interior_inside { 1.0 } else { fill },
            );
            if interior_inside {
                if let Some(effects) = &effects {
                    effects::interior(effects, &inputs, &mut grouped);
                }
                grouped.scale(fill);
            }
            for clip in clips {
                self.draw_clipped(clip, &mut grouped, &shape, backdrops)?;
            }
            if !interior_inside && let Some(effects) = &effects {
                effects::interior(effects, &inputs, &mut grouped);
            }
            composite_raster(target, &grouped, mode, &space, None, 1.0);
        } else {
            composite_raster(target, &masked, mode, &space, None, fill);
            if let Some(effects) = &effects {
                effects::interior(effects, &inputs, target);
            }
        }
        if let Some(before) = &before {
            restrict_channels(&restricted, before, target);
        }
        if let Some(effects) = &effects {
            effects::top(effects, &inputs, target);
        }
        if let Some(region) = &region {
            canvas.lerp_from(region, layer.opacity(), &global_masks);
        }
        Ok(())
    }

    fn draw_clipped(
        &self,
        node: &Node,
        grouped: &mut Raster,
        shape: &Mask,
        backdrops: &Backdrops<'_>,
    ) -> Result<()> {
        if !node.layer().visible() {
            return Ok(());
        }
        if let Node::Layer(layer) = node
            && layer.kind() == LayerKind::Adjustment
        {
            return self.draw_adjustment(layer, grouped, Some(shape));
        }
        let mut drawn = grouped.clone();
        self.draw_node(node, &[], &mut drawn, backdrops)?;
        grouped.lerp_from(&drawn, 1.0, &[shape]);
        Ok(())
    }

    fn blend_if(&self, layer: &Layer, source: &Raster, backdrop: &Raster, area: Rect) -> Mask {
        let space = self.paint.space;
        let ranges = &layer.blending_ranges;
        let mut weights = Mask::new(area, 1.0);
        let mut src = [0.0f32; 4];
        let mut dst = [0.0f32; 4];
        let ramp = |range: &BlendRange, value: f32, source_side: bool| {
            let bounds = if source_side {
                range.source
            } else {
                range.destination
            };
            let v = value * 255.0;
            let [a, b, c, d] = bounds.map(f32::from);
            if v < a || v > d {
                0.0
            } else if v < b {
                if b > a { (v - a) / (b - a) } else { 1.0 }
            } else if v > c {
                if d > c { (d - v) / (d - c) } else { 1.0 }
            } else {
                1.0
            }
        };
        for y in area.top..area.bottom {
            for x in area.left..area.right {
                source.straight(x, y, &mut src);
                backdrop.straight(x, y, &mut dst);
                let mut weight = 1.0;
                if let Some(gray) = &ranges.gray {
                    weight *= ramp(gray, space.luminance(&src), true)
                        * ramp(gray, space.luminance(&dst), false);
                }
                for (channel, range) in ranges.channels.iter().enumerate().take(space.channels()) {
                    weight *= ramp(range, src[channel], true) * ramp(range, dst[channel], false);
                }
                weights.set(x, y, weight);
            }
        }
        weights
    }
}

fn artboard(layer: &Layer, space: &Space) -> Option<(Rect, Option<[f32; 4]>)> {
    let descriptor = layer.descriptor(&[b"artb", b"artd", b"abdd"])?;
    let rect = descriptor.descriptor("artboardRect")?;
    let edge = |key: &str| rect.number(key).unwrap_or(0.0).round() as i32;
    let bounds = Rect::new(edge("Left"), edge("Top "), edge("Rght"), edge("Btom"));
    let background = match descriptor.integer("artboardBackgroundType").unwrap_or(1) {
        1 => Some(space.white()),
        2 => Some(space.color(&rikka_psd::Color::BLACK)),
        3 => None,
        _ => descriptor
            .descriptor("Clr ")
            .and_then(rikka_psd::Color::from_descriptor)
            .map(|color| space.color(&color)),
    };
    Some((bounds, background))
}

fn restricted_channels(layer: &Layer, space: &Space) -> Vec<usize> {
    layer
        .channel_restrictions()
        .iter()
        .filter_map(|&channel| usize::try_from(channel).ok())
        .filter(|&channel| channel < space.channels())
        .collect()
}

fn restrict_channels(restricted: &[usize], before: &Raster, target: &mut Raster) {
    let channels = target.channels;
    for (pixel, old) in target
        .data
        .chunks_exact_mut(channels + 1)
        .zip(before.data.chunks_exact(channels + 1))
    {
        let old_alpha = old[channels];
        let new_alpha = pixel[channels];
        for &channel in restricted {
            pixel[channel] = if old_alpha > 0.0 {
                old[channel] / old_alpha * new_alpha
            } else {
                0.0
            };
        }
    }
}

fn mask_coverage(mask: &Mask) -> Option<Rect> {
    coverage(mask.bounds, &mask.data, 1, 1e-3)
}

fn raster_coverage(raster: &Raster) -> Option<Rect> {
    coverage(raster.bounds, &raster.data, raster.stride(), 0.0)
}

fn coverage(bounds: Rect, data: &[f32], stride: usize, threshold: f32) -> Option<Rect> {
    let width = bounds.width() as usize;
    if width == 0 {
        return None;
    }
    let (mut left, mut top, mut right, mut bottom) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for (y, row) in data.chunks_exact(width * stride).enumerate() {
        let mut covered = row[stride - 1..]
            .iter()
            .step_by(stride)
            .map(|&value| value > threshold);
        let (Some(first), Some(last)) = (
            covered.clone().position(|inside| inside),
            covered.rposition(|inside| inside),
        ) else {
            continue;
        };
        left = left.min(first);
        right = right.max(last + 1);
        top = top.min(y);
        bottom = y + 1;
    }
    (left != usize::MAX).then(|| {
        Rect::new(
            bounds.left + left as i32,
            bounds.top + top as i32,
            bounds.left + right as i32,
            bounds.top + bottom as i32,
        )
    })
}
