use crate::error::{Error, Result};
use crate::io::{Reader, Writer};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Knot {
    pub linked: bool,
    pub preceding: Point,
    pub anchor: Point,
    pub leaving: Point,
}

impl Knot {
    pub fn corner(anchor: Point) -> Self {
        Self {
            linked: false,
            preceding: anchor,
            anchor,
            leaving: anchor,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathOperation {
    Exclude,
    Combine,
    Subtract,
    Intersect,
    Unspecified(i16),
}

impl PathOperation {
    fn from_code(code: i16) -> Self {
        match code {
            0 => PathOperation::Exclude,
            1 => PathOperation::Combine,
            2 => PathOperation::Subtract,
            3 => PathOperation::Intersect,
            other => PathOperation::Unspecified(other),
        }
    }

    fn code(self) -> i16 {
        match self {
            PathOperation::Exclude => 0,
            PathOperation::Combine => 1,
            PathOperation::Subtract => 2,
            PathOperation::Intersect => 3,
            PathOperation::Unspecified(code) => code,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Subpath {
    pub closed: bool,
    pub operation: PathOperation,
    pub rule: u16,
    pub extra: [u8; 18],
    pub knots: Vec<Knot>,
}

impl Subpath {
    pub fn new(closed: bool, operation: PathOperation, knots: Vec<Knot>) -> Self {
        Self {
            closed,
            operation,
            rule: 1,
            extra: [0; 18],
            knots,
        }
    }

    pub fn origin_index(&self) -> u32 {
        u32::from_be_bytes([self.extra[4], self.extra[5], self.extra[6], self.extra[7]])
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clipboard {
    pub top: f64,
    pub left: f64,
    pub bottom: f64,
    pub right: f64,
    pub resolution: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PathRecord {
    FillRule,
    Clipboard(Clipboard),
    InitialFill { all_pixels: bool },
    Subpath(Subpath),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PathData {
    pub records: Vec<PathRecord>,
}

impl PathData {
    pub fn new(subpaths: Vec<Subpath>) -> Self {
        let mut records = vec![
            PathRecord::FillRule,
            PathRecord::InitialFill { all_pixels: false },
        ];
        records.extend(subpaths.into_iter().map(PathRecord::Subpath));
        Self { records }
    }

    pub fn subpaths(&self) -> impl Iterator<Item = &Subpath> {
        self.records.iter().filter_map(|record| match record {
            PathRecord::Subpath(subpath) => Some(subpath),
            _ => None,
        })
    }

    pub fn subpaths_mut(&mut self) -> impl Iterator<Item = &mut Subpath> {
        self.records.iter_mut().filter_map(|record| match record {
            PathRecord::Subpath(subpath) => Some(subpath),
            _ => None,
        })
    }

    pub fn starts_with_all_pixels(&self) -> bool {
        self.records
            .iter()
            .any(|record| matches!(record, PathRecord::InitialFill { all_pixels: true }))
    }

    pub fn clipboard(&self) -> Option<&Clipboard> {
        self.records.iter().find_map(|record| match record {
            PathRecord::Clipboard(clipboard) => Some(clipboard),
            _ => None,
        })
    }

    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Self> {
        let mut records = Vec::new();
        while reader.remaining() >= 26 {
            let selector = reader.u16()?;
            let mut body = reader.sub(24)?;
            match selector {
                0 | 3 => {
                    let count = body.u16()?;
                    let operation = PathOperation::from_code(body.i16()?);
                    let rule = body.u16()?;
                    let mut extra = [0u8; 18];
                    extra.copy_from_slice(body.bytes(18)?);
                    let closed = selector == 0;
                    let mut knots = Vec::with_capacity(usize::from(count));
                    for _ in 0..count {
                        if reader.remaining() < 26 {
                            return Err(reader.malformed("path knot"));
                        }
                        let kind = reader.u16()?;
                        if !matches!(kind, 1 | 2 | 4 | 5) {
                            return Err(reader.malformed("path knot"));
                        }
                        let mut point = || -> Result<Point> {
                            let y = fixed(reader.i32()?);
                            let x = fixed(reader.i32()?);
                            Ok(Point { x, y })
                        };
                        let preceding = point()?;
                        let anchor = point()?;
                        let leaving = point()?;
                        knots.push(Knot {
                            linked: matches!(kind, 1 | 4),
                            preceding,
                            anchor,
                            leaving,
                        });
                    }
                    records.push(PathRecord::Subpath(Subpath {
                        closed,
                        operation,
                        rule,
                        extra,
                        knots,
                    }));
                }
                1 | 2 | 4 | 5 => return Err(reader.malformed("stray path knot")),
                6 => records.push(PathRecord::FillRule),
                7 => {
                    let top = fixed(body.i32()?);
                    let left = fixed(body.i32()?);
                    let bottom = fixed(body.i32()?);
                    let right = fixed(body.i32()?);
                    let resolution = fixed(body.i32()?);
                    records.push(PathRecord::Clipboard(Clipboard {
                        top,
                        left,
                        bottom,
                        right,
                        resolution,
                    }));
                }
                8 => records.push(PathRecord::InitialFill {
                    all_pixels: body.u16()? != 0,
                }),
                other => {
                    return Err(Error::Invalid {
                        what: "path record selector",
                        value: i64::from(other),
                    });
                }
            }
        }
        Ok(Self { records })
    }

    pub(crate) fn write(&self, writer: &mut Writer) {
        for record in &self.records {
            match record {
                PathRecord::Subpath(subpath) => {
                    writer.u16(if subpath.closed { 0 } else { 3 });
                    writer.u16(subpath.knots.len() as u16);
                    writer.i16(subpath.operation.code());
                    writer.u16(subpath.rule);
                    writer.bytes(&subpath.extra);
                    for knot in &subpath.knots {
                        let kind = match (subpath.closed, knot.linked) {
                            (true, true) => 1,
                            (true, false) => 2,
                            (false, true) => 4,
                            (false, false) => 5,
                        };
                        writer.u16(kind);
                        for point in [knot.preceding, knot.anchor, knot.leaving] {
                            writer.i32(to_fixed(point.y));
                            writer.i32(to_fixed(point.x));
                        }
                    }
                }
                PathRecord::FillRule => {
                    writer.u16(6);
                    writer.zeros(24);
                }
                PathRecord::Clipboard(clipboard) => {
                    writer.u16(7);
                    for value in [
                        clipboard.top,
                        clipboard.left,
                        clipboard.bottom,
                        clipboard.right,
                        clipboard.resolution,
                    ] {
                        writer.i32(to_fixed(value));
                    }
                    writer.zeros(4);
                }
                PathRecord::InitialFill { all_pixels } => {
                    writer.u16(8);
                    writer.u16(u16::from(*all_pixels));
                    writer.zeros(22);
                }
            }
        }
    }
}

fn fixed(value: i32) -> f64 {
    f64::from(value) / f64::from(1 << 24)
}

fn to_fixed(value: f64) -> i32 {
    (value * f64::from(1 << 24))
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}
