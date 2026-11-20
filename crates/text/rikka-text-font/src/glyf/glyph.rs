use crate::outline::bounds::Bounds;
use crate::parse::bytes::Bytes;
use crate::parse::cursor::Cursor;
use crate::sfnt::glyph_id::GlyphId;
use rikka_types::Point;

pub(crate) const ON_CURVE: u8 = 0x01;
const X_SHORT: u8 = 0x02;
const Y_SHORT: u8 = 0x04;
const REPEAT: u8 = 0x08;
const X_SAME_OR_POSITIVE: u8 = 0x10;
const Y_SAME_OR_POSITIVE: u8 = 0x20;

const ARGS_ARE_WORDS: u16 = 0x0001;
const ARGS_ARE_XY_VALUES: u16 = 0x0002;
const HAVE_SCALE: u16 = 0x0008;
const MORE_COMPONENTS: u16 = 0x0020;
const HAVE_XY_SCALE: u16 = 0x0040;
const HAVE_TWO_BY_TWO: u16 = 0x0080;
const USE_MY_METRICS: u16 = 0x0200;
const SCALED_COMPONENT_OFFSET: u16 = 0x0800;
const UNSCALED_COMPONENT_OFFSET: u16 = 0x1000;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Glyph<'a> {
    Empty,
    Simple(SimpleGlyph<'a>),
    Composite(CompositeGlyph<'a>),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SimpleGlyph<'a> {
    pub(crate) bounds: Bounds,
    pub(crate) contour_ends: &'a [u8],
    pub(crate) point_count: usize,
    pub(crate) body: Cursor<'a>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CompositeGlyph<'a> {
    pub(crate) bounds: Bounds,
    pub(crate) components: &'a [u8],
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Anchor {
    Offset(f32, f32),
    Points { parent: u16, child: u16 },
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Component {
    pub(crate) glyph: GlyphId,
    pub(crate) anchor: Anchor,
    pub(crate) matrix: [f32; 4],
    pub(crate) scaled_offset: bool,
    pub(crate) use_my_metrics: bool,
}

pub(crate) fn header_bounds(data: &[u8]) -> Option<Bounds> {
    Some(Bounds {
        x_min: f32::from(data.i16_at(2)?),
        y_min: f32::from(data.i16_at(4)?),
        x_max: f32::from(data.i16_at(6)?),
        y_max: f32::from(data.i16_at(8)?),
    })
}

impl<'a> Glyph<'a> {
    pub(crate) fn parse(data: &'a [u8]) -> Option<Self> {
        if data.is_empty() {
            return Some(Self::Empty);
        }
        let contour_count = data.i16_at(0)?;
        let bounds = header_bounds(data)?;
        if contour_count < 0 {
            return Some(Self::Composite(CompositeGlyph {
                bounds,
                components: data.tail(10)?,
            }));
        }
        let contour_count = contour_count as usize;
        let contour_ends = data.range(10, contour_count * 2)?;
        let point_count = match contour_ends.len() {
            0 => 0,
            len => usize::from(contour_ends.u16_at(len - 2)?) + 1,
        };
        let instructions_at = 10 + contour_count * 2;
        let instruction_len = usize::from(data.u16_at(instructions_at)?);
        let body = Cursor::from_position(data, instructions_at + 2 + instruction_len);
        Some(Self::Simple(SimpleGlyph {
            bounds,
            contour_ends,
            point_count,
            body,
        }))
    }
}

impl SimpleGlyph<'_> {
    pub(crate) fn read_points(
        &self,
        flags_out: &mut Vec<u8>,
        points_out: &mut Vec<Point>,
    ) -> Option<()> {
        let flags_start = flags_out.len();
        let mut cursor = self.body;
        while flags_out.len() - flags_start < self.point_count {
            let flag = cursor.u8()?;
            flags_out.push(flag);
            if flag & REPEAT != 0 {
                let repeat = usize::from(cursor.u8()?);
                let room = self.point_count - (flags_out.len() - flags_start);
                for _ in 0..repeat.min(room) {
                    flags_out.push(flag);
                }
            }
        }
        let flags = flags_out.get(flags_start..)?;
        let points_start = points_out.len();
        let mut x = 0i32;
        for &flag in flags {
            x += read_coordinate(&mut cursor, flag, X_SHORT, X_SAME_OR_POSITIVE)?;
            points_out.push(Point::new(x as f32, 0.0));
        }
        let mut y = 0i32;
        for (&flag, point) in flags.iter().zip(points_out.get_mut(points_start..)?) {
            y += read_coordinate(&mut cursor, flag, Y_SHORT, Y_SAME_OR_POSITIVE)?;
            point.y = y as f32;
        }
        Some(())
    }
}

#[inline]
fn read_coordinate(
    cursor: &mut Cursor<'_>,
    flag: u8,
    short: u8,
    same_or_positive: u8,
) -> Option<i32> {
    if flag & short != 0 {
        let value = i32::from(cursor.u8()?);
        Some(if flag & same_or_positive != 0 {
            value
        } else {
            -value
        })
    } else if flag & same_or_positive != 0 {
        Some(0)
    } else {
        cursor.i16().map(i32::from)
    }
}

impl<'a> CompositeGlyph<'a> {
    pub(crate) fn components(&self) -> Components<'a> {
        Components {
            cursor: Cursor::new(self.components),
            done: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Components<'a> {
    cursor: Cursor<'a>,
    done: bool,
}

impl Iterator for Components<'_> {
    type Item = Component;

    fn next(&mut self) -> Option<Component> {
        if self.done {
            return None;
        }
        let component = self.read();
        if component.is_none() {
            self.done = true;
        }
        component
    }
}

impl Components<'_> {
    fn read(&mut self) -> Option<Component> {
        let cursor = &mut self.cursor;
        let flags = cursor.u16()?;
        let glyph = GlyphId(cursor.u16()?);
        let xy = flags & ARGS_ARE_XY_VALUES != 0;
        let anchor = match (flags & ARGS_ARE_WORDS != 0, xy) {
            (true, true) => Anchor::Offset(f32::from(cursor.i16()?), f32::from(cursor.i16()?)),
            (false, true) => Anchor::Offset(f32::from(cursor.i8()?), f32::from(cursor.i8()?)),
            (true, false) => Anchor::Points {
                parent: cursor.u16()?,
                child: cursor.u16()?,
            },
            (false, false) => Anchor::Points {
                parent: u16::from(cursor.u8()?),
                child: u16::from(cursor.u8()?),
            },
        };
        let f2dot14 = |cursor: &mut Cursor<'_>| cursor.i16().map(|v| f32::from(v) / 16384.0);
        let matrix = if flags & HAVE_SCALE != 0 {
            let scale = f2dot14(cursor)?;
            [scale, 0.0, 0.0, scale]
        } else if flags & HAVE_XY_SCALE != 0 {
            let x_scale = f2dot14(cursor)?;
            let y_scale = f2dot14(cursor)?;
            [x_scale, 0.0, 0.0, y_scale]
        } else if flags & HAVE_TWO_BY_TWO != 0 {
            [
                f2dot14(cursor)?,
                f2dot14(cursor)?,
                f2dot14(cursor)?,
                f2dot14(cursor)?,
            ]
        } else {
            [1.0, 0.0, 0.0, 1.0]
        };
        if flags & MORE_COMPONENTS == 0 {
            self.done = true;
        }
        Some(Component {
            glyph,
            anchor,
            matrix,
            scaled_offset: flags & SCALED_COMPONENT_OFFSET != 0
                && flags & UNSCALED_COMPONENT_OFFSET == 0,
            use_my_metrics: flags & USE_MY_METRICS != 0,
        })
    }
}

impl Component {
    #[inline]
    pub(crate) fn transform(&self, x: f32, y: f32) -> (f32, f32) {
        let [xx, xy, yx, yy] = self.matrix;
        (x * xx + y * yx, x * xy + y * yy)
    }

    #[inline]
    pub(crate) fn is_identity(&self) -> bool {
        self.matrix == [1.0, 0.0, 0.0, 1.0]
    }
}
