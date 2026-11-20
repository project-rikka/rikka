use std::cell::OnceCell;

use rikka_psd::{
    Bevel, BevelStyle, BevelTechnique, BlendMode, Contour, Effects, FillContent, Glow, GlowPaint,
    GlowSource, GlowTechnique, Overlay, Rect, Satin, Shadow, Stroke, StrokePosition,
};

use crate::blend::dissolve_visible;
use crate::composite::{composite_color, composite_raster};
use crate::mask::Mask;
use crate::paint::{GradientLut, PaintContext};
use crate::raster::Raster;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Light {
    pub(crate) angle: f64,
    pub(crate) altitude: f64,
}

impl Light {
    fn resolve(self, use_global: bool, angle: f64, altitude: f64) -> Light {
        if use_global {
            self
        } else {
            Light { angle, altitude }
        }
    }
}

pub(crate) struct EffectInputs<'a> {
    shape: &'a Mask,
    content_bounds: Rect,
    paint: &'a PaintContext<'a>,
    light: Light,
    inverted: OnceCell<Mask>,
    signed_distance: OnceCell<Vec<f32>>,
}

impl<'a> EffectInputs<'a> {
    pub(crate) fn new(
        shape: &'a Mask,
        content_bounds: Rect,
        paint: &'a PaintContext<'a>,
        light: Light,
    ) -> Self {
        Self {
            shape,
            content_bounds,
            paint,
            light,
            inverted: OnceCell::new(),
            signed_distance: OnceCell::new(),
        }
    }

    fn inverted(&self) -> &Mask {
        self.inverted.get_or_init(|| self.shape.inverted())
    }

    fn signed_distance(&self) -> &[f32] {
        self.signed_distance
            .get_or_init(|| self.shape.signed_distance())
    }
}

const MAX_REACH: f64 = 30_375.0;

pub(crate) fn reach(effects: &Effects) -> i32 {
    let mut reach = 0.0f64;
    for shadow in effects.drop_shadows.iter().filter(|e| e.common.enabled) {
        reach = reach.max(shadow.distance + shadow.size * 1.5);
    }
    for glow in effects.outer_glows.iter().filter(|e| e.common.enabled) {
        reach = reach.max(glow.size * 1.5);
    }
    for stroke in effects.strokes.iter().filter(|e| e.common.enabled) {
        reach = reach.max(stroke.size);
    }
    for bevel in effects.bevels.iter().filter(|e| e.enabled) {
        if matches!(
            bevel.style,
            BevelStyle::Outer | BevelStyle::Emboss | BevelStyle::PillowEmboss
        ) {
            reach = reach.max(bevel.size + bevel.soften);
        }
    }
    margin(reach)
}

pub(crate) fn margin(distance: f64) -> i32 {
    distance.clamp(0.0, MAX_REACH).ceil() as i32 + 2
}

fn offset(angle: f64, distance: f64) -> (f32, f32) {
    let radians = angle.to_radians();
    (
        (-radians.cos() * distance) as f32,
        (radians.sin() * distance) as f32,
    )
}

fn contoured(mask: &mut Mask, contour: &Contour) {
    if !contour.is_linear() {
        mask.with_lut(&contour.table());
    }
}

fn apply_noise(mask: &mut Mask, noise: f64) {
    if noise <= 0.0 {
        return;
    }
    let bounds = mask.bounds;
    for y in bounds.top..bounds.bottom {
        for x in bounds.left..bounds.right {
            let value = mask.get(x, y);
            if value > 0.0 && !dissolve_visible(x, y, 1.0 - noise as f32 * 0.5) {
                mask.set(x, y, value * (1.0 - noise as f32));
            }
        }
    }
}

fn spread_blur(source: &Mask, size: f64, spread: f64) -> Mask {
    let spread_pixels = (size * spread) as f32;
    let blur = (size * (1.0 - spread)) as f32;
    source.dilated(spread_pixels).blurred(blur / 2.0)
}

fn paint_coverage(
    inputs: &EffectInputs<'_>,
    target: &mut Raster,
    coverage: &Mask,
    content: &FillContent,
    mode: BlendMode,
    opacity: f64,
) {
    match content {
        FillContent::Solid(color) => composite_color(
            target,
            coverage,
            inputs.paint.space.color(color),
            mode,
            &inputs.paint.space,
            opacity as f32,
        ),
        other => {
            let raster = inputs
                .paint
                .paint(other, coverage.bounds, inputs.content_bounds);
            composite_raster(
                target,
                &raster,
                mode,
                &inputs.paint.space,
                Some(coverage),
                opacity as f32,
            );
        }
    }
}

fn cast_shadow(shadow: &Shadow, inner: bool, inputs: &EffectInputs<'_>, target: &mut Raster) {
    let light = inputs
        .light
        .resolve(shadow.use_global_light, shadow.angle, inputs.light.altitude);
    let (dx, dy) = offset(light.angle, shadow.distance);
    let source = if inner {
        inputs.inverted()
    } else {
        inputs.shape
    };
    let mut mask = spread_blur(source, shadow.size, shadow.spread).shifted(dx, dy);
    contoured(&mut mask, &shadow.contour);
    apply_noise(&mut mask, shadow.noise);
    if inner {
        mask.multiply(inputs.shape);
    } else if shadow.layer_knocks_out {
        mask.multiply(inputs.inverted());
    }
    composite_color(
        target,
        &mask,
        inputs.paint.space.color(&shadow.color),
        shadow.common.blend_mode,
        &inputs.paint.space,
        shadow.common.opacity as f32,
    );
}

fn glow_paint(glow: &Glow, inputs: &EffectInputs<'_>, target: &mut Raster, mask: &Mask) {
    match &glow.paint {
        GlowPaint::Color(color) => {
            composite_color(
                target,
                mask,
                inputs.paint.space.color(color),
                glow.common.blend_mode,
                &inputs.paint.space,
                glow.common.opacity as f32,
            );
        }
        GlowPaint::Gradient(gradient) => {
            let lut = GradientLut::new(gradient, &inputs.paint.space, false);
            let channels = inputs.paint.space.channels();
            let mut raster = Raster::new(mask.bounds, channels);
            let bounds = mask.bounds;
            for y in bounds.top..bounds.bottom {
                for x in bounds.left..bounds.right {
                    let value = mask.get(x, y);
                    let (color, alpha) = lut.at(1.0 - value);
                    let index = raster.index(x, y).expect("inside");
                    let alpha = alpha * value;
                    for (slot, value) in raster.data[index..index + channels].iter_mut().zip(&color)
                    {
                        *slot = value * alpha;
                    }
                    raster.data[index + channels] = alpha;
                }
            }
            composite_raster(
                target,
                &raster,
                glow.common.blend_mode,
                &inputs.paint.space,
                None,
                glow.common.opacity as f32,
            );
        }
    }
}

fn precise(distance: &[f32], bounds: Rect, size: f64, spread: f64) -> Mask {
    let mut mask = Mask::new(bounds, 0.0);
    let solid = (size * spread) as f32;
    let fade = ((size * (1.0 - spread)) as f32).max(0.5);
    for (slot, &value) in mask.data.iter_mut().zip(distance) {
        *slot = (1.0 - (value - solid) / fade).clamp(0.0, 1.0);
    }
    mask
}

fn outer_glow(glow: &Glow, inputs: &EffectInputs<'_>, target: &mut Raster) {
    let size = glow.size;
    let mut mask = match glow.technique {
        GlowTechnique::Softer => spread_blur(inputs.shape, size, glow.spread),
        GlowTechnique::Precise => {
            let distance = inputs.shape.distance_outside();
            let mut mask = precise(&distance, inputs.shape.bounds, size, glow.spread);
            for (slot, &coverage) in mask.data.iter_mut().zip(&inputs.shape.data) {
                *slot = slot.max(coverage);
            }
            mask
        }
    };
    contoured(&mut mask, &glow.contour);
    apply_noise(&mut mask, glow.noise);
    mask.multiply(inputs.inverted());
    glow_paint(glow, inputs, target, &mask);
}

fn inner_glow(glow: &Glow, inputs: &EffectInputs<'_>, target: &mut Raster) {
    let size = glow.size;
    let mut mask = match glow.technique {
        GlowTechnique::Softer => spread_blur(inputs.inverted(), size, glow.spread),
        GlowTechnique::Precise => {
            let distance = inputs.shape.distance_inside();
            precise(&distance, inputs.shape.bounds, size, glow.spread)
        }
    };
    if glow.source == GlowSource::Center {
        mask = mask.inverted();
    }
    contoured(&mut mask, &glow.contour);
    apply_noise(&mut mask, glow.noise);
    mask.multiply(inputs.shape);
    glow_paint(glow, inputs, target, &mask);
}

fn satin(satin: &Satin, inputs: &EffectInputs<'_>, target: &mut Raster) {
    let (dx, dy) = offset(satin.angle, satin.distance);
    let sigma = (satin.size / 2.0) as f32;
    let first = inputs.shape.shifted(dx, dy).blurred(sigma);
    let second = inputs.shape.shifted(-dx, -dy).blurred(sigma);
    let mut mask = Mask::new(inputs.shape.bounds, 0.0);
    for ((slot, a), b) in mask.data.iter_mut().zip(&first.data).zip(&second.data) {
        *slot = (a - b).abs();
    }
    contoured(&mut mask, &satin.contour);
    if satin.invert {
        mask = mask.inverted();
    }
    mask.multiply(inputs.shape);
    composite_color(
        target,
        &mask,
        inputs.paint.space.color(&satin.color),
        satin.common.blend_mode,
        &inputs.paint.space,
        satin.common.opacity as f32,
    );
}

fn overlay(overlay: &Overlay, inputs: &EffectInputs<'_>, target: &mut Raster) {
    paint_coverage(
        inputs,
        target,
        inputs.shape,
        &overlay.content,
        overlay.common.blend_mode,
        overlay.common.opacity,
    );
}

fn stroke(stroke: &Stroke, inputs: &EffectInputs<'_>, target: &mut Raster) {
    let size = (stroke.size) as f32;
    let distance = inputs.signed_distance();
    let mut mask = Mask::new(inputs.shape.bounds, 0.0);
    let (low, high) = match stroke.position {
        StrokePosition::Outside => (0.0, size),
        StrokePosition::Inside => (-size, 0.0),
        StrokePosition::Center => (-size / 2.0, size / 2.0),
    };
    for (slot, &signed) in mask.data.iter_mut().zip(distance) {
        *slot = (high - signed + 0.5).clamp(0.0, 1.0) * (signed - low + 0.5).clamp(0.0, 1.0);
    }
    paint_coverage(
        inputs,
        target,
        &mask,
        &stroke.content,
        stroke.common.blend_mode,
        stroke.common.opacity,
    );
}

fn bevel(bevel: &Bevel, inputs: &EffectInputs<'_>, target: &mut Raster) {
    let size = (bevel.size).max(1.0) as f32;
    let bounds = inputs.shape.bounds;
    let distance = inputs.signed_distance();
    let mut height = Mask::new(bounds, 0.0);
    let mut region = Mask::new(bounds, 0.0);
    for (index, &signed) in distance.iter().enumerate() {
        let coverage = inputs.shape.data[index];
        let (h, r) = match bevel.style {
            BevelStyle::Inner | BevelStyle::StrokeEmboss => {
                ((-signed / size).clamp(0.0, 1.0), coverage)
            }
            BevelStyle::Outer => (
                if signed <= 0.0 {
                    1.0
                } else {
                    (1.0 - signed / size).clamp(0.0, 1.0)
                },
                1.0 - coverage,
            ),
            BevelStyle::Emboss => (
                (0.5 - signed / (2.0 * size)).clamp(0.0, 1.0),
                if signed.abs() <= size + 1.0 { 1.0 } else { 0.0 },
            ),
            BevelStyle::PillowEmboss => (
                (signed.abs() / size).clamp(0.0, 1.0),
                if signed.abs() <= size + 1.0 { 1.0 } else { 0.0 },
            ),
        };
        height.data[index] = h;
        region.data[index] = r;
    }
    if bevel.use_shape && !bevel.shape_contour.is_linear() {
        height.with_lut(&bevel.shape_contour.table());
    }
    height = match bevel.technique {
        BevelTechnique::Smooth => height.blurred(size / 4.0),
        BevelTechnique::ChiselSoft => height.blurred(1.0),
        BevelTechnique::ChiselHard => height,
    };
    let light = inputs
        .light
        .resolve(bevel.use_global_light, bevel.angle, bevel.altitude);
    let (angle, altitude) = (light.angle.to_radians(), light.altitude.to_radians());
    let light = [
        (altitude.cos() * angle.cos()) as f32,
        (-altitude.cos() * angle.sin()) as f32,
        altitude.sin() as f32,
    ];
    let flat = light[2];
    let direction = if bevel.up { 1.0 } else { -1.0 };
    let relief = bevel.depth as f32 * size * direction;
    let mut highlight = Mask::new(bounds, 0.0);
    let mut shadow = Mask::new(bounds, 0.0);
    let width = bounds.width() as i32;
    let gloss = (!bevel.gloss_contour.is_linear()).then(|| bevel.gloss_contour.table());
    for y in bounds.top..bounds.bottom {
        for x in bounds.left..bounds.right {
            let gx = (height.get((x + 1).min(bounds.left + width - 1), y)
                - height.get((x - 1).max(bounds.left), y))
                * 0.5;
            let gy = (height.get(x, (y + 1).min(bounds.bottom - 1))
                - height.get(x, (y - 1).max(bounds.top)))
                * 0.5;
            let normal = [-gx * relief, -gy * relief, 1.0];
            let length = (normal[0] * normal[0] + normal[1] * normal[1] + 1.0).sqrt();
            let mut shade =
                (normal[0] * light[0] + normal[1] * light[1] + normal[2] * light[2]) / length;
            if let Some(table) = &gloss {
                shade = table[(shade.clamp(0.0, 1.0) * 255.0).round() as usize];
            }
            let lit = ((shade - flat) / (1.0 - flat).max(1e-3)).clamp(0.0, 1.0);
            let dark = ((flat - shade) / flat.max(1e-3)).clamp(0.0, 1.0);
            let r = region.get(x, y);
            highlight.set(x, y, lit * r);
            shadow.set(x, y, dark * r);
        }
    }
    if bevel.soften > 0.0 {
        let sigma = (bevel.soften / 2.0) as f32;
        highlight = highlight.blurred(sigma);
        shadow = shadow.blurred(sigma);
    }
    let space = &inputs.paint.space;
    composite_color(
        target,
        &shadow,
        space.color(&bevel.shadow_color),
        bevel.shadow_mode,
        space,
        bevel.shadow_opacity as f32,
    );
    composite_color(
        target,
        &highlight,
        space.color(&bevel.highlight_color),
        bevel.highlight_mode,
        space,
        bevel.highlight_opacity as f32,
    );
}

pub(crate) fn outer(effects: &Effects, inputs: &EffectInputs<'_>, target: &mut Raster) {
    for shadow in effects
        .drop_shadows
        .iter()
        .rev()
        .filter(|e| e.common.enabled)
    {
        cast_shadow(shadow, false, inputs, target);
    }
    for glow in effects
        .outer_glows
        .iter()
        .rev()
        .filter(|e| e.common.enabled)
    {
        outer_glow(glow, inputs, target);
    }
}

pub(crate) fn interior(effects: &Effects, inputs: &EffectInputs<'_>, target: &mut Raster) {
    for item in effects
        .pattern_overlays
        .iter()
        .rev()
        .filter(|e| e.common.enabled)
    {
        overlay(item, inputs, target);
    }
    for item in effects
        .gradient_overlays
        .iter()
        .rev()
        .filter(|e| e.common.enabled)
    {
        overlay(item, inputs, target);
    }
    for item in effects
        .color_overlays
        .iter()
        .rev()
        .filter(|e| e.common.enabled)
    {
        overlay(item, inputs, target);
    }
    for item in effects.satins.iter().rev().filter(|e| e.common.enabled) {
        satin(item, inputs, target);
    }
    for glow in effects
        .inner_glows
        .iter()
        .rev()
        .filter(|e| e.common.enabled)
    {
        inner_glow(glow, inputs, target);
    }
    for shadow in effects
        .inner_shadows
        .iter()
        .rev()
        .filter(|e| e.common.enabled)
    {
        cast_shadow(shadow, true, inputs, target);
    }
}

pub(crate) fn top(effects: &Effects, inputs: &EffectInputs<'_>, target: &mut Raster) {
    for item in effects.strokes.iter().rev().filter(|e| e.common.enabled) {
        stroke(item, inputs, target);
    }
    for item in effects.bevels.iter().rev().filter(|e| e.enabled) {
        bevel(item, inputs, target);
    }
}
