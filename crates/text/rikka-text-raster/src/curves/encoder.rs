use rikka_text_font::{Bounds, Point, Transform};

use crate::curves::blob::{BandAxis, CurveBlob};
use crate::curves::quadratics::{Quadratic, quadratics};
use crate::outline::path::Outline;

const CUBIC_TOLERANCE_EM: f32 = 1.0 / 16384.0;
const CURVES_PER_BAND: usize = 2;
const BAND_SLACK: f32 = 1.0 / 65536.0;

#[derive(Debug, Default)]
pub(crate) struct CurveEncoder {
    quadratics: Vec<Quadratic>,
    extents: Vec<Bounds>,
    band: Vec<(f32, u16)>,
}

impl CurveEncoder {
    pub(crate) fn encode(
        &mut self,
        outline: &Outline,
        to_em: &Transform,
        blob: &mut CurveBlob,
    ) -> bool {
        blob.words.clear();
        quadratics(outline, to_em, CUBIC_TOLERANCE_EM, &mut self.quadratics);
        let count = self.quadratics.len();
        if count == 0 || count > usize::from(u16::MAX) {
            return false;
        }
        let bounds = self.bounds();
        let width = bounds.x_max - bounds.x_min;
        let height = bounds.y_max - bounds.y_min;
        let usable = width > 0.0 && height > 0.0 && width.is_finite() && height.is_finite();
        if !usable {
            return false;
        }
        let bands = count
            .div_ceil(CURVES_PER_BAND)
            .clamp(1, CurveBlob::MAX_BANDS);
        let curves_word = CurveBlob::BAND_HEADERS_WORD + 2 * bands;
        if curves_word + count * CurveBlob::CURVE_WORDS > CurveBlob::MAX_WORDS {
            return false;
        }
        let words = &mut blob.words;
        words.push(
            (count as u32) << CurveBlob::CURVE_COUNT_SHIFT
                | (bands as u32) << CurveBlob::HORIZONTAL_BANDS_SHIFT
                | (bands as u32) << CurveBlob::VERTICAL_BANDS_SHIFT,
        );
        for value in [bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max] {
            words.push(value.to_bits());
        }
        words.resize(curves_word, 0);
        self.extents.clear();
        for curve in &self.quadratics {
            let mut extent = Bounds::EMPTY;
            for point in [curve.p0, curve.p1, curve.p2] {
                let x = quantize(point.x, bounds.x_min, width);
                let y = quantize(point.y, bounds.y_min, height);
                words.push(x | y << 16);
                let x = dequantize(x, bounds.x_min, width);
                let y = dequantize(y, bounds.y_min, height);
                extent.include(x, y);
            }
            self.extents.push(extent);
        }
        for axis in [BandAxis::Horizontal, BandAxis::Vertical] {
            let (low, span) = match axis {
                BandAxis::Horizontal => (bounds.y_min, height),
                BandAxis::Vertical => (bounds.x_min, width),
            };
            let slack = span * BAND_SLACK;
            for band in 0..bands {
                let start = low + span * band as f32 / bands as f32 - slack;
                let end = low + span * (band + 1) as f32 / bands as f32 + slack;
                self.band.clear();
                for (index, extent) in self.extents.iter().enumerate() {
                    let (min, max, key, flat) = match axis {
                        BandAxis::Horizontal => (
                            extent.y_min,
                            extent.y_max,
                            extent.x_max,
                            extent.y_min == extent.y_max,
                        ),
                        BandAxis::Vertical => (
                            extent.x_min,
                            extent.x_max,
                            extent.y_max,
                            extent.x_min == extent.x_max,
                        ),
                    };
                    if !flat && max >= start && min <= end {
                        let offset = curves_word + index * CurveBlob::CURVE_WORDS;
                        self.band.push((key, offset as u16));
                    }
                }
                self.band
                    .sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
                let offset = words.len();
                if offset + self.band.len().div_ceil(CurveBlob::REFERENCES_PER_WORD)
                    > CurveBlob::MAX_WORDS
                {
                    words.clear();
                    return false;
                }
                let header = match axis {
                    BandAxis::Horizontal => CurveBlob::BAND_HEADERS_WORD + band,
                    BandAxis::Vertical => CurveBlob::BAND_HEADERS_WORD + bands + band,
                };
                words[header] =
                    offset as u32 | (self.band.len() as u32) << CurveBlob::BAND_COUNT_SHIFT;
                for pair in self.band.chunks(CurveBlob::REFERENCES_PER_WORD) {
                    let low = u32::from(pair[0].1);
                    let high = pair.get(1).map_or(0, |entry| u32::from(entry.1));
                    words.push(low | high << 16);
                }
            }
        }
        words.len() <= CurveBlob::MAX_WORDS
    }

    fn bounds(&self) -> Bounds {
        let mut bounds = Bounds::EMPTY;
        for curve in &self.quadratics {
            for point in [curve.p0, curve.p1, curve.p2] {
                bounds.include(point.x, point.y);
            }
        }
        bounds
    }
}

#[inline]
fn quantize(value: f32, min: f32, size: f32) -> u32 {
    ((value - min) / size * CurveBlob::UNORM_MAX)
        .round()
        .clamp(0.0, CurveBlob::UNORM_MAX) as u32
}

#[inline]
pub(crate) fn dequantize(value: u32, min: f32, size: f32) -> f32 {
    min + value as f32 / CurveBlob::UNORM_MAX * size
}

pub(crate) fn unpack(word: u32, bounds: &Bounds) -> Point {
    Point::new(
        dequantize(word & 0xFFFF, bounds.x_min, bounds.x_max - bounds.x_min),
        dequantize(word >> 16, bounds.y_min, bounds.y_max - bounds.y_min),
    )
}
