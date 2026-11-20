use rikka_image_resample::{Axis, Footprint, Job, Resampler, Threads};
use rikka_png::{Decoder as PngDecoder, Limits};
use rikka_text_font::{BitmapData, BitmapGlyph, BitmapMetrics, Font, GlyphId, SbixFormat};

use crate::bitmap::decoded_strikes::{Decoded, DecodedStrikes, StrikeKey, StrikeTable};
use crate::color::srgb::{Encoder, straight_rgba8_to_linear, to_byte};
use crate::image::{GlyphImage, GlyphImageKind, Placement};

const MAX_COMPONENTS: usize = 64;
const MAX_SIDE: usize = 1024;
const PNG_LIMITS: Limits = Limits {
    max_width: 16384,
    max_height: 16384,
    max_bytes: 4 << 24,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct BitmapTarget {
    pub(crate) ppem: f32,
    pub(crate) subpixel_x: f32,
    pub(crate) subpixel_y: f32,
}

#[derive(Debug, Default)]
pub(crate) struct BitmapRenderer {
    png: PngDecoder,
    component: Vec<u8>,
    coverage: Vec<f32>,
    strikes: DecodedStrikes,
    resampled: Vec<f32>,
    resampler: Resampler,
}

impl BitmapRenderer {
    pub(crate) fn render_strike(
        &mut self,
        font: &Font<'_>,
        glyph: GlyphId,
        target: BitmapTarget,
        image: &mut GlyphImage,
    ) -> bool {
        let Some(bitmap) = font.bitmap_glyph(glyph, requested_ppem(target.ppem)) else {
            return false;
        };
        if bitmap.ppem_x == 0 || bitmap.ppem_y == 0 {
            return false;
        }
        if let BitmapData::Mask { data, bit_aligned } = bitmap.data {
            return self.render_mask(&bitmap, data, bit_aligned, target, image);
        }
        let key = strike_key(
            font,
            glyph,
            u16::from(bitmap.ppem_x) << 8 | u16::from(bitmap.ppem_y),
            StrikeTable::Cbdt,
        );
        let Self {
            png,
            component,
            strikes,
            resampled,
            resampler,
            coverage,
        } = self;
        let decoded = strikes.get_or_decode(key, |pixels| {
            decode_strike_glyph(font, &bitmap, png, component, coverage, pixels)
        });
        let Some((decoded, pixels)) = decoded else {
            return false;
        };
        finish_color(resampler, resampled, decoded, pixels, target, image)
    }

    pub(crate) fn render_sbix(
        &mut self,
        font: &Font<'_>,
        glyph: GlyphId,
        target: BitmapTarget,
        image: &mut GlyphImage,
    ) -> bool {
        let Some(bitmap) = font
            .sbix()
            .and_then(|sbix| sbix.glyph(glyph, requested_ppem(target.ppem)))
        else {
            return false;
        };
        if bitmap.format != SbixFormat::Png || bitmap.ppem == 0 {
            return false;
        }
        let key = strike_key(font, glyph, bitmap.ppem, StrikeTable::Sbix);
        let Self {
            png,
            strikes,
            resampled,
            resampler,
            ..
        } = self;
        let decoded = strikes.get_or_decode(key, |pixels| {
            let (width, height) = decode_png(png, bitmap.data, pixels)?;
            Some(Decoded {
                width,
                height,
                left: f32::from(bitmap.origin_x),
                top: f32::from(bitmap.origin_y) + height as f32,
                strike_ppem_x: f32::from(bitmap.ppem),
                strike_ppem_y: f32::from(bitmap.ppem),
            })
        });
        let Some((decoded, pixels)) = decoded else {
            return false;
        };
        finish_color(resampler, resampled, decoded, pixels, target, image)
    }

    fn render_mask(
        &mut self,
        bitmap: &BitmapGlyph<'_>,
        data: &[u8],
        bit_aligned: bool,
        target: BitmapTarget,
        image: &mut GlyphImage,
    ) -> bool {
        let metrics = bitmap.metrics;
        if mask_coverage(
            data,
            metrics,
            bitmap.bit_depth,
            bit_aligned,
            &mut self.coverage,
        )
        .is_none()
        {
            return false;
        }
        let decoded = Decoded {
            width: usize::from(metrics.width),
            height: usize::from(metrics.height),
            left: f32::from(metrics.bearing_x),
            top: f32::from(metrics.bearing_y),
            strike_ppem_x: f32::from(bitmap.ppem_x),
            strike_ppem_y: f32::from(bitmap.ppem_y),
        };
        let (x, y, placement) = match place(decoded, target, GlyphImageKind::Mask, image) {
            Some(Placed::Scaled { x, y, placement }) => (x, y, placement),
            Some(Placed::Empty) => return true,
            None => return false,
        };
        let coverage = self.coverage.as_slice();
        let columns = x.source_len;
        resample::<1>(
            &mut self.resampler,
            &mut self.resampled,
            (x, y),
            |row, out| out.copy_from_slice(&coverage[row * columns..(row + 1) * columns]),
        );
        let pixels = image.prepare(GlyphImageKind::Mask, placement);
        for (target, &value) in pixels.iter_mut().zip(&self.resampled) {
            *target = to_byte(value);
        }
        true
    }
}

fn strike_key(font: &Font<'_>, glyph: GlyphId, ppem: u16, table: StrikeTable) -> StrikeKey {
    StrikeKey {
        font: font.data().as_ptr() as usize,
        len: font.data().len(),
        face: font.face_index(),
        glyph: glyph.0,
        ppem,
        table,
    }
}

fn decode_png(png: &mut PngDecoder, data: &[u8], pixels: &mut Vec<u8>) -> Option<(usize, usize)> {
    let (width, height) = png.rgba8(data, &PNG_LIMITS, pixels).ok()?;
    Some((width as usize, height as usize))
}

fn decode_strike_glyph(
    font: &Font<'_>,
    bitmap: &BitmapGlyph<'_>,
    png: &mut PngDecoder,
    component: &mut Vec<u8>,
    coverage: &mut Vec<f32>,
    pixels: &mut Vec<u8>,
) -> Option<Decoded> {
    let metrics = bitmap.metrics;
    let mut decoded = Decoded {
        width: usize::from(metrics.width),
        height: usize::from(metrics.height),
        left: f32::from(metrics.bearing_x),
        top: f32::from(metrics.bearing_y),
        strike_ppem_x: f32::from(bitmap.ppem_x),
        strike_ppem_y: f32::from(bitmap.ppem_y),
    };
    match bitmap.data {
        BitmapData::Png(data) => {
            let (width, height) = decode_png(png, data, pixels)?;
            decoded.width = width;
            decoded.height = height;
        }
        BitmapData::Mask { .. } => return None,
        BitmapData::Composite(components) => {
            let strike = font.bitmap_strikes()?.iter().find(|strike| {
                strike.ppem_x() == bitmap.ppem_x && strike.ppem_y() == bitmap.ppem_y
            })?;
            pixels.clear();
            pixels.resize(decoded.width * decoded.height * 4, 0);
            for part in components.iter().take(MAX_COMPONENTS) {
                let Some(glyph) = strike.glyph(part.glyph) else {
                    continue;
                };
                let Some((width, height)) = decode_component(&glyph, png, component, coverage)
                else {
                    continue;
                };
                blit(
                    component,
                    (width, height),
                    pixels,
                    &decoded,
                    part.x_offset,
                    part.y_offset,
                );
            }
        }
    }
    Some(decoded)
}

fn decode_component(
    part: &BitmapGlyph<'_>,
    png: &mut PngDecoder,
    component: &mut Vec<u8>,
    coverage: &mut Vec<f32>,
) -> Option<(usize, usize)> {
    match part.data {
        BitmapData::Png(data) => decode_png(png, data, component),
        BitmapData::Mask { data, bit_aligned } => {
            mask_coverage(data, part.metrics, part.bit_depth, bit_aligned, coverage)?;
            component.clear();
            component.extend(coverage.iter().flat_map(|&value| [0, 0, 0, to_byte(value)]));
            Some((
                usize::from(part.metrics.width),
                usize::from(part.metrics.height),
            ))
        }
        BitmapData::Composite(_) => None,
    }
}

fn blit(
    component: &[u8],
    (width, height): (usize, usize),
    pixels: &mut [u8],
    canvas: &Decoded,
    x_offset: i8,
    y_offset: i8,
) {
    for row in 0..height {
        let y = row as isize + isize::from(y_offset);
        if y < 0 || y as usize >= canvas.height {
            continue;
        }
        for column in 0..width {
            let x = column as isize + isize::from(x_offset);
            if x < 0 || x as usize >= canvas.width {
                continue;
            }
            let from = (row * width + column) * 4;
            let to = (y as usize * canvas.width + x as usize) * 4;
            if component[from + 3] > 0 {
                pixels[to..to + 4].copy_from_slice(&component[from..from + 4]);
            }
        }
    }
}

enum Placed {
    Empty,
    Scaled {
        x: Axis,
        y: Axis,
        placement: Placement,
    },
}

fn place(
    decoded: Decoded,
    target: BitmapTarget,
    kind: GlyphImageKind,
    image: &mut GlyphImage,
) -> Option<Placed> {
    if decoded.width == 0 || decoded.height == 0 {
        image.clear(kind);
        return Some(Placed::Empty);
    }
    let scale_x = target.ppem / decoded.strike_ppem_x;
    let scale_y = target.ppem / decoded.strike_ppem_y;
    let valid = scale_x.is_finite() && scale_y.is_finite() && scale_x > 0.0 && scale_y > 0.0;
    if !valid {
        return None;
    }
    let (left, x) = Axis::place(
        decoded.left * scale_x + target.subpixel_x,
        decoded.width,
        scale_x,
    );
    let (top, y) = Axis::place(
        -decoded.top * scale_y - target.subpixel_y,
        decoded.height,
        scale_y,
    );
    if x.target_len > MAX_SIDE || y.target_len > MAX_SIDE {
        return None;
    }
    let placement = Placement {
        left,
        top: -top,
        width: x.target_len as u32,
        height: y.target_len as u32,
    };
    Some(Placed::Scaled { x, y, placement })
}

fn resample<const C: usize>(
    resampler: &mut Resampler,
    resampled: &mut Vec<f32>,
    (x, y): (Axis, Axis),
    load: impl Fn(usize, &mut [f32]) + Sync,
) {
    resampled.clear();
    resampled.resize(x.target_len * y.target_len * C, 0.0);
    let job = Job {
        x,
        y,
        footprint: Footprint::Coverage,
        threads: Threads::Calling,
    };
    resampler.resample::<f32, C>(job, resampled, load, |blended, out| {
        out.copy_from_slice(blended);
    });
}

fn finish_color(
    resampler: &mut Resampler,
    resampled: &mut Vec<f32>,
    decoded: Decoded,
    straight: &[u8],
    target: BitmapTarget,
    image: &mut GlyphImage,
) -> bool {
    let (x, y, placement) = match place(decoded, target, GlyphImageKind::Color, image) {
        Some(Placed::Scaled { x, y, placement }) => (x, y, placement),
        Some(Placed::Empty) => return true,
        None => return false,
    };
    let pitch = x.source_len * 4;
    resample::<4>(resampler, resampled, (x, y), |row, out| {
        let texels = straight[row * pitch..(row + 1) * pitch].as_chunks::<4>().0;
        for (linear, &texel) in out.as_chunks_mut::<4>().0.iter_mut().zip(texels) {
            *linear = straight_rgba8_to_linear(texel);
        }
    });
    let pixels = image.prepare(GlyphImageKind::Color, placement);
    Encoder::new()
        .write_linear_premultiplied(resampled.as_chunks::<4>().0, pixels.as_chunks_mut::<4>().0);
    true
}

fn requested_ppem(ppem: f32) -> u16 {
    ppem.ceil().clamp(1.0, f32::from(u16::MAX)) as u16
}

fn mask_coverage(
    data: &[u8],
    metrics: BitmapMetrics,
    bit_depth: u8,
    bit_aligned: bool,
    out: &mut Vec<f32>,
) -> Option<()> {
    let (width, height) = (usize::from(metrics.width), usize::from(metrics.height));
    let depth = usize::from(bit_depth);
    if !matches!(depth, 1 | 2 | 4 | 8) {
        return None;
    }
    let maximum = ((1u32 << depth) - 1) as f32;
    let row_bits = if bit_aligned {
        width * depth
    } else {
        (width * depth).next_multiple_of(8)
    };
    out.clear();
    for row in 0..height {
        for column in 0..width {
            let bit = row * row_bits + column * depth;
            let byte = *data.get(bit / 8)?;
            let shift = 8 - depth - bit % 8;
            let value = (byte >> shift) & ((1u16 << depth) - 1) as u8;
            out.push(f32::from(value) / maximum);
        }
    }
    Some(())
}
