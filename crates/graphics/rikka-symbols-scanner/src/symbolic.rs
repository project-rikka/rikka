use std::fmt;
use std::path::Path;

use rikka_text_font::{OutlineScratch, OutlineSink, Tag};

use crate::coverage;
use crate::error::{Error, Result};
use crate::listing::Icon;

const AXES: [(Tag, f32); 4] = [
    (Tag::new(b"FILL"), 1.0),
    (Tag::new(b"wght"), 400.0),
    (Tag::new(b"GRAD"), 0.0),
    (Tag::new(b"opsz"), 24.0),
];

const ICON_SIZE: u32 = 16;

pub(crate) fn document(path: &Path, bytes: &[u8], icon: &Icon) -> Result<String> {
    let font = coverage::open_font(path, bytes)?;
    let missing_outline = || Error::MissingOutline {
        path: path.to_owned(),
        name: icon.name.clone(),
    };
    let glyph = font
        .charmap()
        .and_then(|charmap| coverage::glyph(&charmap, icon))
        .ok_or_else(missing_outline)?;
    let mut path_data = PathData::default();
    font.outline(
        glyph,
        &font.location(AXES),
        &mut OutlineScratch::new(),
        &mut path_data,
    )
    .map_err(|_| missing_outline())?;
    if path_data.is_empty() {
        return Err(missing_outline());
    }
    let em = font.units_per_em();
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{ICON_SIZE}\" height=\"{ICON_SIZE}\" viewBox=\"0 -{em} {em} {em}\"><path d=\"{path_data}\"/></svg>\n"
    ))
}

#[derive(Default)]
struct PathData(Vec<Segment>);

enum Segment {
    Move(Point),
    Line(Point),
    Quad(Point, Point),
    Cubic(Point, Point, Point),
    Close,
}

#[derive(Clone, Copy)]
struct Point {
    x: f32,
    y: f32,
}

impl PathData {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl OutlineSink for PathData {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(Segment::Move(Point { x, y }));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(Segment::Line(Point { x, y }));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0
            .push(Segment::Quad(Point { x: x1, y: y1 }, Point { x, y }));
    }

    fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.push(Segment::Cubic(
            Point { x: x1, y: y1 },
            Point { x: x2, y: y2 },
            Point { x, y },
        ));
    }

    fn close(&mut self) {
        self.0.push(Segment::Close);
    }
}

impl fmt::Display for PathData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for segment in &self.0 {
            match segment {
                Segment::Move(to) => write!(f, "M{to}")?,
                Segment::Line(to) => write!(f, "L{to}")?,
                Segment::Quad(control, to) => write!(f, "Q{control} {to}")?,
                Segment::Cubic(first, second, to) => write!(f, "C{first} {second} {to}")?,
                Segment::Close => f.write_str("Z")?,
            }
        }
        Ok(())
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", Coordinate(self.x), Coordinate(-self.y))
    }
}

struct Coordinate(f32);

impl fmt::Display for Coordinate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hundredths = (self.0 * 100.0).round() as i64;
        let sign = if hundredths < 0 { "-" } else { "" };
        let (whole, fraction) = (hundredths.abs() / 100, hundredths.abs() % 100);
        match fraction {
            0 => write!(f, "{sign}{whole}"),
            tenths if tenths % 10 == 0 => write!(f, "{sign}{whole}.{}", tenths / 10),
            _ => write!(f, "{sign}{whole}.{fraction:02}"),
        }
    }
}
