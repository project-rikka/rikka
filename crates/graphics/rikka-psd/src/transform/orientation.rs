use crate::error::{Error, Result};
use crate::geometry::Rect;
use crate::pixels::{Depth, Plane, Samples};
use crate::transform::Affine;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuarterTurn {
    Clockwise,
    Half,
    CounterClockwise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Orientation {
    swap: bool,
    flip_x: bool,
    flip_y: bool,
}

impl From<Axis> for Orientation {
    fn from(axis: Axis) -> Self {
        Self {
            swap: false,
            flip_x: axis == Axis::Horizontal,
            flip_y: axis == Axis::Vertical,
        }
    }
}

impl From<QuarterTurn> for Orientation {
    fn from(turn: QuarterTurn) -> Self {
        match turn {
            QuarterTurn::Clockwise => Self {
                swap: true,
                flip_x: false,
                flip_y: true,
            },
            QuarterTurn::Half => Self {
                swap: false,
                flip_x: true,
                flip_y: true,
            },
            QuarterTurn::CounterClockwise => Self {
                swap: true,
                flip_x: true,
                flip_y: false,
            },
        }
    }
}

impl Orientation {
    pub(crate) fn size(self, width: u32, height: u32) -> (u32, u32) {
        if self.swap {
            (height, width)
        } else {
            (width, height)
        }
    }

    pub(crate) fn canvas(self, width: u32, height: u32) -> Affine {
        let (width, height) = (f64::from(width), f64::from(height));
        let mirror = |flip: bool, extent: f64| {
            if flip { (-1.0, extent) } else { (1.0, 0.0) }
        };
        if self.swap {
            let (along_x, shift_x) = mirror(self.flip_y, height);
            let (along_y, shift_y) = mirror(self.flip_x, width);
            Affine {
                xx: 0.0,
                xy: along_y,
                yx: along_x,
                yy: 0.0,
                tx: shift_x,
                ty: shift_y,
            }
        } else {
            let (along_x, shift_x) = mirror(self.flip_x, width);
            let (along_y, shift_y) = mirror(self.flip_y, height);
            Affine {
                xx: along_x,
                xy: 0.0,
                yx: 0.0,
                yy: along_y,
                tx: shift_x,
                ty: shift_y,
            }
        }
    }

    pub(crate) fn rect(self, rect: Rect, width: u32, height: u32) -> Result<Rect> {
        let mirror = |flip: bool, extent: u32, low: i32, high: i32| -> Result<(i32, i32)> {
            if !flip {
                return Ok((low, high));
            }
            let extent = i64::from(extent);
            let mirrored =
                |edge: i32| i32::try_from(extent - i64::from(edge)).map_err(|_| Error::Oversized);
            Ok((mirrored(high)?, mirrored(low)?))
        };
        let (left, right) = mirror(self.flip_x, width, rect.left, rect.right)?;
        let (top, bottom) = mirror(self.flip_y, height, rect.top, rect.bottom)?;
        Ok(if self.swap {
            Rect::new(top, left, bottom, right)
        } else {
            Rect::new(left, top, right, bottom)
        })
    }

    pub(crate) fn plane(self, plane: &Plane) -> Result<Plane> {
        let (width, height) = (plane.width() as usize, plane.height() as usize);
        let (to_width, to_height) = self.size(plane.width(), plane.height());
        let source = |column: usize, row: usize| {
            let (u, v) = if self.swap {
                (row, column)
            } else {
                (column, row)
            };
            let x = if self.flip_x { width - 1 - u } else { u };
            let y = if self.flip_y { height - 1 - v } else { v };
            (x, y)
        };
        fn remap<T: Copy>(
            values: &[T],
            stride: usize,
            size: (u32, u32),
            source: impl Fn(usize, usize) -> (usize, usize),
        ) -> Vec<T> {
            let (width, height) = (size.0 as usize, size.1 as usize);
            let mut out = Vec::with_capacity(width * height);
            for row in 0..height {
                for column in 0..width {
                    let (x, y) = source(column, row);
                    out.push(values[y * stride + x]);
                }
            }
            out
        }
        let size = (to_width, to_height);
        let samples = match plane.samples() {
            Samples::U8(values) => Samples::U8(remap(values, width, size, source)),
            Samples::U16(values) => Samples::U16(remap(values, width, size, source)),
            Samples::F32(values) => Samples::F32(remap(values, width, size, source)),
            Samples::Bits(bits) => {
                let from = Depth::One.row_bytes(plane.width());
                let to = Depth::One.row_bytes(to_width);
                let mut out = vec![0u8; to * to_height as usize];
                for row in 0..to_height as usize {
                    for column in 0..to_width as usize {
                        let (x, y) = source(column, row);
                        if bits[y * from + x / 8] & (0x80 >> (x % 8)) != 0 {
                            out[row * to + column / 8] |= 0x80 >> (column % 8);
                        }
                    }
                }
                Samples::Bits(out)
            }
        };
        Plane::new(to_width, to_height, samples)
    }
}

impl Plane {
    pub fn flipped(&self, axis: Axis) -> Result<Plane> {
        Orientation::from(axis).plane(self)
    }

    pub fn rotated_quarter(&self, turn: QuarterTurn) -> Result<Plane> {
        Orientation::from(turn).plane(self)
    }
}
