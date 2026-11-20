use crate::color::color_line::{ColorStops, Extend};
use crate::color::color_value::ColorIndex;
use crate::color::colr_variations::ColrVariations;
use crate::color::composite_mode::CompositeMode;
use crate::color::transform::Transform;
use crate::parse::bytes::Bytes;
use crate::sfnt::glyph_id::GlyphId;
use rikka_types::Point;

const NO_VARIATION: u32 = u32::MAX;
const HALF_TURN_DEGREES: f32 = 180.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PaintId(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub enum Paint<'c> {
    ColrLayers {
        first: u32,
        count: u8,
    },
    Solid {
        color: ColorIndex,
        alpha: f32,
    },
    LinearGradient {
        stops: ColorStops<'c>,
        extend: Extend,
        p0: Point,
        p1: Point,
        p2: Point,
    },
    RadialGradient {
        stops: ColorStops<'c>,
        extend: Extend,
        c0: Point,
        r0: f32,
        c1: Point,
        r1: f32,
    },
    SweepGradient {
        stops: ColorStops<'c>,
        extend: Extend,
        center: Point,
        start_degrees: f32,
        end_degrees: f32,
    },
    Glyph {
        glyph: GlyphId,
        paint: PaintId,
    },
    ColrGlyph {
        glyph: GlyphId,
    },
    Transform {
        transform: Transform,
        paint: PaintId,
    },
    Composite {
        source: PaintId,
        mode: CompositeMode,
        backdrop: PaintId,
    },
}

struct Fields<'c> {
    data: &'c [u8],
    id: u32,
    base: u32,
    variations: ColrVariations<'c>,
}

impl<'c> Fields<'c> {
    fn child(&self, at: usize) -> Option<PaintId> {
        let offset = self.data.u24_at(at)?;
        if offset == 0 {
            return None;
        }
        Some(PaintId(self.id.checked_add(offset)?))
    }

    fn fword(&self, at: usize, field: u32) -> Option<f32> {
        Some(
            self.variations
                .fword(self.data.i16_at(at)?, self.base, field),
        )
    }

    fn ufword(&self, at: usize, field: u32) -> Option<f32> {
        Some(
            self.variations
                .ufword(self.data.u16_at(at)?, self.base, field),
        )
    }

    fn f2dot14(&self, at: usize, field: u32) -> Option<f32> {
        Some(
            self.variations
                .f2dot14(self.data.i16_at(at)?, self.base, field),
        )
    }

    fn point(&self, at: usize, field: u32) -> Option<Point> {
        Some(Point::new(
            self.fword(at, field)?,
            self.fword(at + 2, field + 1)?,
        ))
    }

    fn base_at(&self, variable: bool, at: usize) -> Option<u32> {
        if variable {
            self.data.u32_at(at)
        } else {
            Some(NO_VARIATION)
        }
    }

    fn color_line(&self, table: &'c [u8], variable: bool) -> Option<(ColorStops<'c>, Extend)> {
        let offset = self.data.u24_at(1)?;
        if offset == 0 {
            return None;
        }
        let line = table.tail((self.id as usize).checked_add(offset as usize)?)?;
        ColorStops::parse(line, variable, self.variations)
    }
}

pub(crate) fn parse_paint<'c>(
    table: &'c [u8],
    id: PaintId,
    variations: ColrVariations<'c>,
) -> Option<Paint<'c>> {
    let data = table.tail(id.0 as usize)?;
    let format = data.u8_at(0)?;
    let variable = format >= 3 && format % 2 == 1;
    let mut fields = Fields {
        data,
        id: id.0,
        base: NO_VARIATION,
        variations,
    };
    let base_offset = match format {
        3 => 5,
        5 | 7 => 16,
        9 => 12,
        15 | 17 | 29 => 8,
        19 | 31 => 12,
        21 | 25 => 6,
        23 | 27 => 10,
        _ => 0,
    };
    fields.base = fields.base_at(variable && base_offset != 0, base_offset)?;
    let paint = match format {
        1 => Paint::ColrLayers {
            count: data.u8_at(1)?,
            first: data.u32_at(2)?,
        },
        2 | 3 => Paint::Solid {
            color: ColorIndex::from_raw(data.u16_at(1)?),
            alpha: fields.f2dot14(3, 0)?,
        },
        4 | 5 => {
            let (stops, extend) = fields.color_line(table, variable)?;
            Paint::LinearGradient {
                stops,
                extend,
                p0: fields.point(4, 0)?,
                p1: fields.point(8, 2)?,
                p2: fields.point(12, 4)?,
            }
        }
        6 | 7 => {
            let (stops, extend) = fields.color_line(table, variable)?;
            Paint::RadialGradient {
                stops,
                extend,
                c0: fields.point(4, 0)?,
                r0: fields.ufword(8, 2)?,
                c1: fields.point(10, 3)?,
                r1: fields.ufword(14, 5)?,
            }
        }
        8 | 9 => {
            let (stops, extend) = fields.color_line(table, variable)?;
            Paint::SweepGradient {
                stops,
                extend,
                center: fields.point(4, 0)?,
                start_degrees: (fields.f2dot14(8, 2)? + 1.0) * HALF_TURN_DEGREES,
                end_degrees: (fields.f2dot14(10, 3)? + 1.0) * HALF_TURN_DEGREES,
            }
        }
        10 => Paint::Glyph {
            paint: fields.child(1)?,
            glyph: GlyphId(data.u16_at(4)?),
        },
        11 => Paint::ColrGlyph {
            glyph: GlyphId(data.u16_at(1)?),
        },
        12 | 13 => Paint::Transform {
            paint: fields.child(1)?,
            transform: affine(table, &fields, variable)?,
        },
        14..=31 => Paint::Transform {
            paint: fields.child(1)?,
            transform: transform(format, &fields)?,
        },
        32 => Paint::Composite {
            source: fields.child(1)?,
            mode: CompositeMode::from_raw(data.u8_at(4)?),
            backdrop: fields.child(5)?,
        },
        _ => return None,
    };
    Some(paint)
}

fn affine(table: &[u8], fields: &Fields<'_>, variable: bool) -> Option<Transform> {
    let offset = fields.data.u24_at(4)?;
    if offset == 0 {
        return None;
    }
    let matrix = table.tail((fields.id as usize).checked_add(offset as usize)?)?;
    let base = if variable {
        matrix.u32_at(24)?
    } else {
        NO_VARIATION
    };
    let value = |index: u32| -> Option<f32> {
        let raw = matrix.i32_at(index as usize * 4)?;
        Some(fields.variations.fixed(raw, base, index))
    };
    Some(Transform {
        xx: value(0)?,
        yx: value(1)?,
        xy: value(2)?,
        yy: value(3)?,
        dx: value(4)?,
        dy: value(5)?,
    })
}

fn transform(format: u8, fields: &Fields<'_>) -> Option<Transform> {
    let degrees = |at: usize, field: u32| -> Option<f32> {
        Some(fields.f2dot14(at, field)? * HALF_TURN_DEGREES)
    };
    let transform = match format {
        14 | 15 => Transform::translate(fields.fword(4, 0)?, fields.fword(6, 1)?),
        16 | 17 => Transform::scale(fields.f2dot14(4, 0)?, fields.f2dot14(6, 1)?),
        18 | 19 => Transform::scale(fields.f2dot14(4, 0)?, fields.f2dot14(6, 1)?)
            .around(fields.point(8, 2)?),
        20 | 21 => {
            let scale = fields.f2dot14(4, 0)?;
            Transform::scale(scale, scale)
        }
        22 | 23 => {
            let scale = fields.f2dot14(4, 0)?;
            Transform::scale(scale, scale).around(fields.point(6, 1)?)
        }
        24 | 25 => Transform::rotate(degrees(4, 0)?),
        26 | 27 => Transform::rotate(degrees(4, 0)?).around(fields.point(6, 1)?),
        28 | 29 => Transform::skew(degrees(4, 0)?, degrees(6, 1)?),
        30 | 31 => Transform::skew(degrees(4, 0)?, degrees(6, 1)?).around(fields.point(8, 2)?),
        _ => return None,
    };
    Some(transform)
}
