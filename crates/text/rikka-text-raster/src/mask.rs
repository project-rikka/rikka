use rikka_simd::{Float, Level, Simd};
use rikka_text_font::Transform;
use rikka_vector::{GLYPH_FLATNESS, Polylines, Rasterizer};

use crate::device::device;
use crate::image::{GlyphImage, GlyphImageKind, Placement};
use crate::outline::flattened::Flattened;
use crate::outline::path::Outline;
use crate::outline::prepared::PreparedOutline;

#[derive(Debug, Default)]
pub(crate) struct MaskRasterizer {
    rasterizer: Rasterizer,
    steps: Vec<u16>,
}

impl MaskRasterizer {
    pub(crate) fn new(level: Level) -> Self {
        Self {
            rasterizer: Rasterizer::with_level(level),
            steps: Vec::new(),
        }
    }

    #[inline]
    pub(crate) fn level(&self) -> Level {
        self.rasterizer.level()
    }

    pub(crate) fn fill(
        &mut self,
        outline: &Outline,
        transform: &Transform,
        image: &mut GlyphImage,
    ) -> bool {
        if self
            .rasterizer
            .flatten(outline.path(), &device(&flip(transform)), GLYPH_FLATNESS)
            .is_err()
        {
            return false;
        }
        let rasterizer = &mut self.rasterizer;
        rikka_simd::dispatch!(rasterizer.level(), simd => rasterize(simd, rasterizer, image))
    }

    pub(crate) fn fill_prepared(
        &mut self,
        prepared: &mut PreparedOutline,
        transform: &Transform,
        embolden: f32,
        image: &mut GlyphImage,
    ) -> bool {
        let device = flip(transform);
        rikka_simd::dispatch!(
            self.level(),
            simd => self.place(simd, prepared, &device, embolden, image)
        )
    }

    #[inline(always)]
    fn place<S: Simd>(
        &mut self,
        simd: S,
        prepared: &mut PreparedOutline,
        device: &Transform,
        embolden: f32,
        image: &mut GlyphImage,
    ) -> bool {
        let needs_miters = embolden != 0.0;
        let Some(flattened) = prepared.flattened(
            size_class(device),
            GLYPH_FLATNESS,
            needs_miters,
            &mut self.steps,
        ) else {
            return false;
        };
        place::<S::F32s>(self.rasterizer.polylines_mut(), flattened, device, embolden);
        rasterize(simd, &mut self.rasterizer, image)
    }
}

#[inline(always)]
fn rasterize<S: Simd>(simd: S, rasterizer: &mut Rasterizer, image: &mut GlyphImage) -> bool {
    let target = &mut *image;
    let filled = rasterizer.mask_in(simd, move |area| {
        target.prepare_overwritten(
            GlyphImageKind::Mask,
            Placement {
                left: area.left,
                top: -area.top,
                width: area.width,
                height: area.height,
            },
        )
    });
    match filled {
        Ok(Some(_)) => true,
        Ok(None) => {
            image.clear(GlyphImageKind::Mask);
            true
        }
        Err(_) => false,
    }
}

#[inline(always)]
fn place<V: Float<Elem = f32>>(
    polylines: &mut Polylines,
    flattened: &Flattened,
    transform: &Transform,
    bold: f32,
) {
    let len = flattened.xs.len().min(flattened.ys.len());
    let (xs, ys, ends) = polylines.columns(len);
    ends.clear();
    ends.extend_from_slice(&flattened.ends);
    let (xx, xy, dx) = (
        V::splat(transform.xx),
        V::splat(transform.xy),
        V::splat(transform.dx),
    );
    let (yx, yy, dy) = (
        V::splat(transform.yx),
        V::splat(transform.yy),
        V::splat(transform.dy),
    );
    let emboldens = flattened.has_miters() && bold != 0.0;
    let bold = V::splat(bold);
    for at in (0..len).step_by(V::LANES) {
        let mut x = V::load(&flattened.xs[at..]);
        let mut y = V::load(&flattened.ys[at..]);
        if emboldens {
            x += V::load(&flattened.miter_xs[at..]) * bold;
            y += V::load(&flattened.miter_ys[at..]) * bold;
        }
        (xx * x + xy * y + dx).store(&mut xs[at..]);
        (yx * x + yy * y + dy).store(&mut ys[at..]);
    }
}

fn flip(transform: &Transform) -> Transform {
    Transform {
        yx: -transform.yx,
        yy: -transform.yy,
        dy: -transform.dy,
        ..*transform
    }
}

fn size_class(transform: &Transform) -> i32 {
    let (a, b, c, d) = (transform.xx, transform.xy, transform.yx, transform.yy);
    let frobenius = a * a + b * b + c * c + d * d;
    let determinant = a * d - b * c;
    let spread = (frobenius * frobenius - 4.0 * determinant * determinant)
        .max(0.0)
        .sqrt();
    let stretch = ((frobenius + spread) * 0.5).sqrt();
    let bits = stretch.to_bits();
    let exponent = ((bits >> 23) & 0xFF) as i32 - 127;
    let mantissa = f32::from_bits((bits & 0x007F_FFFF) | 0x3F80_0000);
    let step = if mantissa == 1.0 {
        0
    } else if mantissa <= std::f32::consts::SQRT_2 {
        1
    } else {
        2
    };
    2 * exponent + step
}
